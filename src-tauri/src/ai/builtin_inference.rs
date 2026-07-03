use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;
use std::path::PathBuf;
use std::fs;

pub struct BuiltinInference {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

impl BuiltinInference {
    pub fn new(model_path: PathBuf) -> Result<Self, String> {
        let device = Device::Cpu;
        
        // 1. Load GGUF file
        let mut file = fs::File::open(&model_path).map_err(|e| format!("Failed to open model file: {}", e))?;
        let gguf = candle_core::quantized::gguf_file::Content::read(&mut file)
            .map_err(|e| format!("Failed to read GGUF file: {}", e))?;
        
        // 2. Load model weights
        let model = ModelWeights::from_gguf(gguf, &mut file, &device)
            .map_err(|e| format!("Failed to load model weights: {}", e))?;
        
        // 3. Load tokenizer (Try to extract from GGUF if possible, or expect it next to model)
        // Note: Qwen2 GGUF usually doesn't have the tokenizer in a format candle-transformers can use directly easily without more boilerplate.
        // However, we can try to find a tokenizer.json in the same folder.
        let tokenizer_path = model_path.with_file_name("tokenizer.json");
        let tokenizer = if tokenizer_path.exists() {
            Tokenizer::from_file(tokenizer_path).map_err(|e| format!("Failed to load tokenizer: {}", e))?
        } else {
            // Fallback: This is risky, but we'll try to use a default Qwen2 tokenizer if we can't find one.
            return Err("tokenizer.json not found in the models folder. Please place it next to model.gguf".to_string());
        };

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Returns (generated_text, prompt_token_count, completion_token_count).
    pub async fn generate(&mut self, prompt: &str, max_tokens: usize, app_handle: &tauri::AppHandle, cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<(String, usize, usize), String> {
        use tauri::Emitter;
        use std::sync::atomic::Ordering;
        
        let _ = app_handle.emit("ai-progress", "Tokenizing prompt...");
        let encoding = self.tokenizer.encode(prompt, true).map_err(|e| e.to_string())?;
        let prompt_tokens = encoding.get_ids().to_vec();
        
        if prompt_tokens.is_empty() {
            return Err("Empty prompt".to_string());
        }
        
        let total_prompt_tokens = prompt_tokens.len();
        let _ = app_handle.emit("ai-progress", format!("Prompt tokens: {}", total_prompt_tokens));
        
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Generation cancelled by user".to_string());
        }
        
        // Batch prefill: Forward the entire prompt at once to build KV cache
        let input_ids = Tensor::new(prompt_tokens.as_slice(), &self.device)
            .map_err(|e| e.to_string())?
            .unsqueeze(0).map_err(|e| e.to_string())?;  // Shape: [1, seq_len]
        
        let _ = app_handle.emit("ai-progress", "Prefilling prompt (batch forward)...");
        
        let mut logits = self.model.forward(&input_ids, 0).map_err(|e| e.to_string())?;  // pos=0 for prefill
        logits = logits.squeeze(0).map_err(|e| e.to_string())?;  // [seq_len, vocab]
        let last_logits = logits.get(logits.dim(0).map_err(|e| e.to_string())? - 1).map_err(|e| e.to_string())?;  // Logits of last token in prompt
        
        let mut logits_processor = LogitsProcessor::new(1337, Some(0.0), None);  // Greedy decode
        
        let mut current_token = logits_processor.sample(&last_logits).map_err(|e| e.to_string())?;
        
        let mut generated_text = String::new();
        let mut completion_tokens = 0usize;
        let mut pos = total_prompt_tokens;  // KV cache ready, start from end of prompt
        
        let _ = app_handle.emit("ai-progress", "Prefill done. Starting generation...");
        
        for i in 0..max_tokens {
            if cancel_flag.load(Ordering::SeqCst) {
                return Err("Generation cancelled by user".to_string());
            }
            
            let input = Tensor::new(&[current_token], &self.device).map_err(|e| e.to_string())?
                .unsqueeze(0).map_err(|e| e.to_string())?;  // [1, 1]
            
            let logits = self.model.forward(&input, pos).map_err(|e| e.to_string())?;
            let logits = logits.squeeze(0).map_err(|e| e.to_string())?.squeeze(0).map_err(|e| e.to_string())?;  // [vocab]
            
            pos += 1;
            
            current_token = logits_processor.sample(&logits).map_err(|e| e.to_string())?;
            
            let end_token_1 = self.tokenizer.get_vocab(true).get("<|endoftext|>").copied().unwrap_or(0);
            let end_token_2 = self.tokenizer.get_vocab(true).get("<|im_end|>").copied().unwrap_or(151645);  // Default for Qwen2.5
            
            if current_token == end_token_1 || current_token == end_token_2 {
                break;
            }
            
            let decoded = self.tokenizer.decode(&[current_token], true).map_err(|e| e.to_string())?;
            generated_text.push_str(&decoded);
            completion_tokens += 1;
            
            // Stream token to frontend for real-time UX
            let _ = app_handle.emit("ai-token", decoded.clone());
            
            // Periodic progress
            if (i + 1) % 10 == 0 {
                let _ = app_handle.emit("ai-progress", format!("Generated {} tokens...", i + 1));
                tokio::task::yield_now().await;
            }
        }
        
        let _ = app_handle.emit("ai-progress", "Generation complete.");
        Ok((generated_text, total_prompt_tokens, completion_tokens))
    }
}
