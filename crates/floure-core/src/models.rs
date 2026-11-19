use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ModelManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub size_bytes: u64,
    pub backend: &'static str,
    pub recommended: bool,
    pub is_archive: bool,
    pub filename: Option<&'static str>,
    /// Multi-file releases (Hugging Face repos) fetch each file separately.
    /// Empty means `url` is the payload.
    pub files: &'static [ModelFile],
}

#[derive(Debug, Clone)]
pub struct ModelFile {
    pub url: &'static str,
    /// Name inside the model directory, normalised to what the runtime loads.
    pub dest: &'static str,
    pub size_bytes: u64,
}

const NO_FILES: &[ModelFile] = &[];

pub const MODEL_MANIFEST: &[ModelManifest] = &[
    ModelManifest {
        id: "silero-vad",
        name: "Silero VAD",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx",
        size_bytes: 643_854,
        backend: "vad",
        recommended: true,
        is_archive: false,
        filename: Some("silero_vad.onnx"),
        files: NO_FILES,
    },
    ModelManifest {
        id: "parakeet-tdt-0.6b-v2-int8",
        name: "Parakeet TDT 0.6B v2 (int8)",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
        size_bytes: 482_468_385,
        backend: "parakeet",
        recommended: true,
        is_archive: true,
        filename: None,
        files: NO_FILES,
    },
    ModelManifest {
        id: "parakeet-ultra-int8",
        name: "Parakeet Ultra (int8, multilingual)",
        url: "https://huggingface.co/tpmaerk/parakeet-ultra-sherpa-onnx-int8",
        size_bytes: 670_576_838,
        backend: "parakeet",
        recommended: false,
        is_archive: false,
        filename: None,
        files: &[
            ModelFile {
                url: "https://huggingface.co/tpmaerk/parakeet-ultra-sherpa-onnx-int8/resolve/main/encoder.int8.onnx",
                dest: "encoder.onnx",
                size_bytes: 652_282_347,
            },
            ModelFile {
                url: "https://huggingface.co/tpmaerk/parakeet-ultra-sherpa-onnx-int8/resolve/main/decoder.int8.onnx",
                dest: "decoder.onnx",
                size_bytes: 11_845_275,
            },
            ModelFile {
                url: "https://huggingface.co/tpmaerk/parakeet-ultra-sherpa-onnx-int8/resolve/main/joiner.int8.onnx",
                dest: "joiner.onnx",
                size_bytes: 6_355_277,
            },
            ModelFile {
                url: "https://huggingface.co/tpmaerk/parakeet-ultra-sherpa-onnx-int8/resolve/main/tokens.txt",
                dest: "tokens.txt",
                size_bytes: 93_939,
            },
        ],
    },
    ModelManifest {
        id: "whisper-large-v3-turbo-q5_1",
        name: "Whisper large-v3-turbo (Q5_1)",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-turbo.tar.bz2",
        size_bytes: 563_790_207,
        backend: "whisper",
        recommended: false,
        is_archive: true,
        filename: None,
        files: NO_FILES,
    },
    ModelManifest {
        id: "whisper-base-q5_1",
        name: "Whisper base (Q5_1)",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
        size_bytes: 207_557_382,
        backend: "whisper",
        recommended: false,
        is_archive: true,
        filename: None,
        files: NO_FILES,
    },
    ModelManifest {
        id: "s1-mini-q4_k_m",
        name: "S1-Mini Q4_K_M",
        url: "https://huggingface.co/superwhisper/s1-mini-GGUF/resolve/main/s1-mini-q4_k_m.gguf",
        size_bytes: 484_219_808,
        backend: "llm",
        recommended: true,
        is_archive: false,
        filename: Some("s1-mini-q4_k_m.gguf"),
        files: NO_FILES,
    },
    ModelManifest {
        id: "gemma-3-1b-it-q4_k_m",
        name: "Gemma 3 1B IT (Q4_K_M)",
        url: "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf",
        size_bytes: 806_058_272,
        backend: "llm",
        recommended: false,
        is_archive: false,
        filename: Some("gemma-3-1b-it-q4_k_m.gguf"),
        files: NO_FILES,
    },
];

pub fn find_model(id: &str) -> Option<&'static ModelManifest> {
    MODEL_MANIFEST.iter().find(|m| m.id == id)
}

pub struct ModelManager {
    model_dir: PathBuf,
}

impl ModelManager {
    pub fn new(model_dir: PathBuf) -> Self {
        Self { model_dir }
    }

    pub fn status(&self) -> Vec<ModelStatus> {
        let mut statuses = Vec::new();

        for model in MODEL_MANIFEST {
            let model_dir = self.model_dir.join(model.id);
            // Truth comes from verification, not directory existence: a
            // partial or poisoned dir (e.g. an old 404 body) must never
            // display as "Downloaded".
            let downloaded = self.verify(model.id);
            let size_bytes = if model_dir.exists() {
                walk_dir_size(&model_dir).unwrap_or(0)
            } else {
                0
            };
            statuses.push(ModelStatus {
                name: model.name.to_string(),
                id: model.id.to_string(),
                downloaded,
                path: model_dir.to_string_lossy().to_string(),
                size_bytes,
                url: model.url.to_string(),
                backend: model.backend.to_string(),
                recommended: model.recommended,
            });
        }

        statuses
    }

    pub fn verify(&self, id: &str) -> bool {
        let model_opt = find_model(id);
        if let Some(model) = model_opt {
            verify_model(&self.model_dir, model)
        } else {
            false
        }
    }

    pub fn download(&self, id: &str, progress: impl FnMut(usize, u64)) -> Result<()> {
        let model_opt = find_model(id);
        if let Some(model) = model_opt {
            let target_dir = self.model_dir.join(model.id);
            download_model(model, &target_dir, progress)
        } else {
            Err(anyhow::anyhow!("Model not found: {}", id))
        }
    }
}

/// Stream an HTTP response body into `dest`, reporting progress as
/// `(percent, downloaded_bytes)`. Shared by the archive and single-file
/// branches of [`download_model`]. When `resume_offset > 0` the file is
/// opened in append mode so an interrupted download can be continued.
fn stream_to_file(
    mut reader: impl std::io::Read,
    dest: &Path,
    expected_total: u64,
    resume_offset: u64,
    progress: &mut impl FnMut(usize, u64),
) -> Result<u64> {
    use std::io::Write;

    let mut file = if resume_offset > 0 {
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(dest)?
    } else {
        std::fs::File::create(dest)?
    };
    let mut downloaded: u64 = 0;
    let mut last_percent: usize = usize::MAX;
    let mut chunk = [0u8; 64 * 1024];

    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| anyhow::anyhow!("Stream error: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&chunk[..n])?;
        downloaded += n as u64;
        let total_so_far = resume_offset + downloaded;
        let percent = if expected_total > 0 {
            (total_so_far as f64 / expected_total as f64 * 100.0) as usize
        } else {
            100
        };
        // One guard here fixes every caller at once: without it each ~16KB
        // chunk emits a Tauri event (~30k re-renders per model download).
        if percent != last_percent {
            last_percent = percent;
            progress(percent, total_so_far);
        }
    }
    file.flush()?;
    drop(file);

    Ok(downloaded)
}

/// Target dirs with an in-flight fetch, keyed by path so the pipeline
/// lazy-download and the Models-page button (or repeated PTT presses)
/// serialize on the same payload instead of interleaving two writers.
fn active_downloads() -> &'static std::sync::Mutex<std::collections::HashSet<PathBuf>> {
    static ACTIVE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
        std::sync::OnceLock::new();
    ACTIVE.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

/// RAII release for the key above: covers every exit path, including `?`.
struct DownloadGuard {
    key: PathBuf,
}

/// True while a fetch for this target dir runs in this process. Lets
/// callers fail fast with "still downloading" instead of spawning a
/// second writer that `download_model` would only reject.
/// Delete one model directory. Confined to the models root so a crafted id
/// can never remove anything else.
pub fn delete_model(model_dir: &Path, id: &str) -> Result<()> {
    let target = model_dir.join(id);
    if !target.exists() {
        return Ok(());
    }
    let root = model_dir
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("models directory: {e}"))?;
    let target = target
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("model directory: {e}"))?;
    if !target.starts_with(&root) {
        return Err(anyhow::anyhow!(
            "refusing to delete outside the models directory"
        ));
    }
    std::fs::remove_dir_all(target)?;
    Ok(())
}

pub fn is_downloading(target_dir: &Path) -> bool {
    active_downloads()
        .lock()
        .map(|active| active.contains(target_dir))
        .unwrap_or(false)
}

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = active_downloads().lock() {
            active.remove(&self.key);
        }
    }
}

/// Move the contents of a single top-level folder up into `model_dir`.
/// The sherpa-onnx release archives unpack into one nested directory
/// (e.g. `sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8/`), but the
/// recognizers expect a flat layout inside the model dir.
fn hoist_single_subdir(model_dir: &Path) -> Result<()> {
    let dirs: Vec<PathBuf> = std::fs::read_dir(model_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| p.file_name().map(|n| n != ".downloaded").unwrap_or(true))
        .collect();
    if dirs.len() == 1 {
        let inner = &dirs[0];
        for entry in std::fs::read_dir(inner)? {
            let entry = entry?;
            let dest = model_dir.join(entry.file_name());
            if dest.exists() {
                if dest.is_dir() {
                    std::fs::remove_dir_all(&dest)?;
                } else {
                    std::fs::remove_file(&dest)?;
                }
            }
            std::fs::rename(entry.path(), dest)?;
        }
        std::fs::remove_dir(inner)?;
        eprintln!(
            "[models] hoisted single archive folder -> {}",
            model_dir.display()
        );
    }
    Ok(())
}

/// Whisper archives name their files `<model>-encoder[.int8].onnx`,
/// `<model>-decoder[.int8].onnx` and `<model>-tokens.txt`. Return the
/// path of the best matching file, preferring the int8 variant when both
/// float32 and int8 are shipped in the same archive.
fn pick_whisper_file(model_dir: &Path, kind: &str) -> Result<PathBuf> {
    let suffixes: &[&str] = match kind {
        "encoder" => &["-encoder.int8.onnx", "-encoder.onnx"],
        "decoder" => &["-decoder.int8.onnx", "-decoder.onnx"],
        "tokens" => &["-tokens.txt"],
        _ => unreachable!("pick_whisper_file: unknown kind"),
    };
    let files: Vec<PathBuf> = std::fs::read_dir(model_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            suffixes.iter().any(|s| name.ends_with(s))
        })
        .collect();
    if files.is_empty() {
        return Err(anyhow::anyhow!(
            "Whisper {} file not found in archive dir {}",
            kind,
            model_dir.display()
        ));
    }
    Ok(files
        .into_iter()
        .min_by_key(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            if name.contains(".int8.") { 0 } else { 1 }
        })
        .unwrap())
}

/// Rename archive files to the flat, canonical layout that `verify_model`
/// and the recognizers expect:
/// - parakeet: `encoder.int8.onnx` -> `encoder.onnx`, etc.
/// - whisper: `<m>-encoder[.int8].onnx` -> `whisper-encoder.onnx`,
///   `<m>-decoder[.int8].onnx` -> `whisper-decoder.onnx`,
///   `<m>-tokens.txt` -> `tokens.txt`
fn normalize_extracted(backend: &str, model_dir: &Path) -> Result<()> {
    hoist_single_subdir(model_dir)?;

    match backend {
        "parakeet" => {
            for name in ["encoder", "decoder", "joiner"] {
                let int8 = model_dir.join(format!("{name}.int8.onnx"));
                let plain = model_dir.join(format!("{name}.onnx"));
                if int8.exists() && !plain.exists() {
                    eprintln!("[models] rename {}.int8.onnx -> {}.onnx", name, name);
                    std::fs::rename(&int8, &plain)?;
                } else if int8.exists() && plain.exists() {
                    std::fs::remove_file(&int8)?;
                }
            }
        }
        "whisper" => {
            let encoder = pick_whisper_file(model_dir, "encoder")?;
            let decoder = pick_whisper_file(model_dir, "decoder")?;
            let tokens = pick_whisper_file(model_dir, "tokens")?;
            for (src, dest_name) in [
                (encoder, "whisper-encoder.onnx"),
                (decoder, "whisper-decoder.onnx"),
                (tokens, "tokens.txt"),
            ] {
                let dest = model_dir.join(dest_name);
                if dest.exists() {
                    std::fs::remove_file(&src)?;
                } else {
                    eprintln!(
                        "[models] rename {} -> {}",
                        src.file_name().unwrap().to_string_lossy(),
                        dest_name
                    );
                    std::fs::rename(src, dest)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn download_model(
    model: &ModelManifest,
    target_dir: &Path,
    progress: impl FnMut(usize, u64),
) -> Result<()> {
    if !model.files.is_empty() {
        return download_files(model, target_dir, progress);
    }
    download_single(model, target_dir, progress)
}

/// Fetch a multi-file release (Hugging Face) into the runtime's flat layout.
/// Each file resumes independently; progress is combined by bytes.
fn download_files(
    model: &ModelManifest,
    target_dir: &Path,
    mut progress: impl FnMut(usize, u64),
) -> Result<()> {
    if target_dir.join(".downloaded").exists()
        && verify_model(target_dir.parent().unwrap_or(target_dir), model)
    {
        eprintln!("[models] {} already downloaded, skipping", model.id);
        return Ok(());
    }
    let _ = std::fs::remove_file(target_dir.join(".downloaded"));

    {
        let mut active = active_downloads().lock().unwrap();
        if !active.insert(target_dir.to_path_buf()) {
            return Err(anyhow::anyhow!("{} download already in progress", model.id));
        }
    }
    let _download_guard = DownloadGuard {
        key: target_dir.to_path_buf(),
    };

    std::fs::create_dir_all(target_dir)?;
    let total: u64 = model.files.iter().map(|file| file.size_bytes).sum();
    fetch_files(model.files, target_dir, total, &mut progress)?;

    if !verify_model(target_dir.parent().unwrap_or(target_dir), model) {
        return Err(anyhow::anyhow!(
            "{} files missing after download in {}",
            model.id,
            target_dir.display()
        ));
    }
    std::fs::write(target_dir.join(".downloaded"), b"")?;
    Ok(())
}

fn fetch_files(
    files: &[ModelFile],
    target_dir: &Path,
    total: u64,
    progress: &mut impl FnMut(usize, u64),
) -> Result<()> {
    let mut completed: u64 = 0;
    for file in files {
        let dest = target_dir.join(file.dest);
        let mut offset = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        if offset > file.size_bytes {
            let _ = std::fs::remove_file(&dest);
            offset = 0;
        }
        if offset == file.size_bytes && file.size_bytes > 0 {
            completed += offset;
            continue;
        }
        let mut request = reqwest::blocking::Client::new().get(file.url);
        if offset > 0 {
            request = request.header("Range", format!("bytes={offset}-"));
        }
        let response = request.send()?.error_for_status()?;
        let previous = completed;
        let mut sent = 0u64;
        stream_to_file(
            response,
            &dest,
            file.size_bytes,
            offset,
            &mut |_percent, bytes| {
                sent = bytes;
                let done = previous + bytes;
                let overall = if total > 0 {
                    (done as f64 / total as f64 * 100.0) as usize
                } else {
                    100
                };
                progress(overall, done);
            },
        )?;
        completed = previous + sent;
    }
    Ok(())
}

fn download_single(
    model: &ModelManifest,
    target_dir: &Path,
    mut progress: impl FnMut(usize, u64),
) -> Result<()> {
    let url = model.url;
    let model_dir = target_dir;

    // Only trust the marker if the payload still verifies: a failed or
    // interrupted run can leave a stale marker behind (e.g. verification
    // failed after download), which would otherwise skip recovery forever.
    if model_dir.join(".downloaded").exists()
        && verify_model(model_dir.parent().unwrap_or(model_dir), model)
    {
        eprintln!("[models] {} already downloaded, skipping", model.id);
        return Ok(());
    }
    let _ = std::fs::remove_file(model_dir.join(".downloaded"));

    // Second press while a fetch runs must fail fast, not append a second
    // writer to the same resume file.
    {
        let mut active = active_downloads().lock().unwrap();
        if !active.insert(model_dir.to_path_buf()) {
            return Err(anyhow::anyhow!("{} download already in progress", model.id));
        }
    }
    let _download_guard = DownloadGuard {
        key: model_dir.to_path_buf(),
    };

    eprintln!(
        "[models] downloading {} -> {} ({} bytes, archive={})",
        model.id,
        model_dir.display(),
        model.size_bytes,
        model.is_archive
    );

    std::fs::create_dir_all(model_dir)?;

    // If a previous attempt left a partial file behind (e.g. the app was
    // interrupted mid-download), resume from where it stopped by issuing a
    // Range request, instead of re-downloading the whole model from zero.
    let ext = url.rsplit('.').next().unwrap_or("bin");
    let resume_file: PathBuf = if model.is_archive {
        model_dir.join("model.tar.bz2")
    } else {
        let file_name: String = model
            .filename
            .map(|f| f.to_string())
            .unwrap_or_else(|| format!("{}.{}", model.id, ext));
        model_dir.join(&file_name)
    };
    let resume_offset = std::fs::metadata(&resume_file)
        .map(|m| m.len())
        .unwrap_or(0);

    let expected_total = model.size_bytes;

    // If the payload is already fully on disk, skip the network entirely —
    // this happens when a previous run finished downloading but was killed
    // before extraction, or when the user dropped the archive in manually.
    let payload_complete = resume_offset > 0 && resume_offset >= expected_total;
    if payload_complete {
        eprintln!(
            "[models] {} payload already on disk at full size ({} bytes), skipping download",
            model.id, resume_offset
        );
    } else {
        let mut request = reqwest::blocking::Client::new().get(url);
        if resume_offset > 0 {
            eprintln!(
                "[models] {} partial file found ({} bytes), resuming download",
                model.id, resume_offset
            );
            request = request.header(reqwest::header::RANGE, format!("bytes={}-", resume_offset));
        }

        let response = request
            .send()
            .map_err(|e| anyhow::anyhow!("Failed to start download of {}: {}", url, e))?;

        // 416 Range Not Satisfiable means the requested range starts at or
        // past the end of the file — i.e. the payload is already complete.
        if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            eprintln!(
                "[models] {} server reports range not satisfiable; file already complete",
                model.id
            );
        } else if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Download of {} returned HTTP {}",
                url,
                response.status()
            ));
        } else {
            // A 206 partial-content response honors our Range header and its
            // content-length counts from the resume offset; otherwise the
            // server ignored the range and we restart from the beginning.
            // Derive everything from the effective offset: with a 200
            // response, content-length is the FULL size and must not be
            // added to resume_offset (that would fail the completion
            // check on a correct file).
            let is_partial = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
            let effective_offset = if is_partial { resume_offset } else { 0 };
            let content_len = response
                .content_length()
                .unwrap_or(expected_total.saturating_sub(effective_offset));
            let total = effective_offset + content_len;
            if resume_offset > 0 && !is_partial {
                eprintln!(
                    "[models] {} server ignored Range request, re-downloading from 0",
                    model.id
                );
            }

            eprintln!(
                "[models] {} download started, content-length={:?} (HTTP {}), offset={}",
                model.id,
                response.content_length(),
                response.status(),
                effective_offset
            );

            // Log progress to stderr every 10% even when the caller's progress
            // callback is a no-op, so the terminal shows the download is alive.
            let mut last_pct: usize = 0;
            let mut on_progress = |percent: usize, bytes: u64| {
                if percent >= last_pct + 10 || percent <= 1 {
                    last_pct = percent;
                    eprintln!(
                        "[models] {} download: {}% ({} bytes)",
                        model.id, percent, bytes
                    );
                }
                progress(percent, bytes);
            };

            let downloaded = stream_to_file(
                response,
                &resume_file,
                total,
                effective_offset,
                &mut on_progress,
            )?;

            if effective_offset + downloaded != total {
                return Err(anyhow::anyhow!(
                    "{} download incomplete: {} of {} bytes ({})",
                    model.id,
                    effective_offset + downloaded,
                    total,
                    resume_file.display()
                ));
            }
        }
    }

    // Extract / finalize — shared by the fresh-download, resumed and
    // already-on-disk paths.
    if model.is_archive {
        eprintln!("[models] {} archive saved, extracting...", model.id);
        // Stream through the decoder (a full-RAM read OOMs on ~600MB
        // archives). On failure the payload is deleted: a corrupt file
        // (e.g. an error page saved by an old broken build, then resumed
        // against the fixed URL) must not latch and fail forever.
        let extract_ok = (|| -> Result<()> {
            let file = std::fs::File::open(&resume_file)?;
            let decompressed = bzip2::read::BzDecoder::new(std::io::BufReader::new(file));
            let mut archive = tar::Archive::new(decompressed);
            archive
                .unpack(model_dir)
                .map_err(|e| anyhow::anyhow!("Failed to extract {} archive: {}", model.id, e))?;
            normalize_extracted(model.backend, model_dir)?;
            Ok(())
        })();
        if extract_ok.is_err() {
            let _ = std::fs::remove_file(&resume_file);
        }
        extract_ok?;
        std::fs::remove_file(&resume_file)?;
    } else {
        eprintln!(
            "[models] {} file saved -> {}",
            model.id,
            resume_file.display()
        );
    }

    // Never latch success over broken files: the sentinel is written only
    // if the payload verifies (both callers pass the per-model dir, so the
    // parent join below resolves back to `model_dir`).
    if !verify_model(model_dir.parent().unwrap_or(model_dir), model) {
        let _ = std::fs::remove_file(model_dir.join(".downloaded"));
        return Err(anyhow::anyhow!(
            "{} files missing after download in {}",
            model.id,
            model_dir.display()
        ));
    }

    std::fs::write(model_dir.join(".downloaded"), b"")?;
    eprintln!(
        "[models] {} download complete -> {}",
        model.id,
        model_dir.display()
    );

    Ok(())
}

pub fn verify_model(models_dir: &Path, model: &ModelManifest) -> bool {
    let model_dir = models_dir.join(model.id);
    if !model_dir.exists() {
        return false;
    }

    match model.backend {
        "vad" => {
            // Size check guards against a stale/truncated file (e.g. a 404
            // body saved by an old broken download): such a file passes
            // `exists()` but makes sherpa-onnx throw a C++ exception at
            // VAD creation, which aborts the process.
            model_dir
                .join("silero_vad.onnx")
                .metadata()
                .map(|m| m.len() == model.size_bytes)
                .unwrap_or(false)
        }
        "parakeet" => {
            model_dir.join("encoder.onnx").exists()
                && model_dir.join("decoder.onnx").exists()
                && model_dir.join("joiner.onnx").exists()
                && model_dir.join("tokens.txt").exists()
        }
        "whisper" => {
            model_dir.join("whisper-encoder.onnx").exists()
                && model_dir.join("whisper-decoder.onnx").exists()
                && model_dir.join("tokens.txt").exists()
        }
        "llm" => model_dir.join(format!("{}.gguf", model.id)).exists(),
        _ => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub name: String,
    pub id: String,
    pub downloaded: bool,
    pub path: String,
    pub size_bytes: u64,
    pub url: String,
    pub backend: String,
    pub recommended: bool,
}

pub(crate) fn walk_dir_size(path: &std::path::Path) -> Result<u64, anyhow::Error> {
    let mut total = 0u64;
    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|e| anyhow::anyhow!(e))? {
            let entry = entry.map_err(|e| anyhow::anyhow!(e))?;
            let meta = entry.metadata().map_err(|e| anyhow::anyhow!(e))?;
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                total += walk_dir_size(&entry.path())?;
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod delete_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn delete_is_confined_to_the_models_root() {
        let root = std::env::temp_dir().join(format!(
            "floure-delete-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let model = root.join("parakeet");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("model.onnx"), b"x").unwrap();

        delete_model(&root, "parakeet").unwrap();
        assert!(!model.exists());

        let escape = root
            .parent()
            .unwrap()
            .join(format!("floure-escape-{}", std::process::id()));
        std::fs::create_dir_all(&escape).unwrap();
        assert!(
            delete_model(
                &root,
                &format!("../{}", escape.file_name().unwrap().to_string_lossy())
            )
            .is_err()
        );
        assert!(
            escape.exists(),
            "the guard must not delete outside the root"
        );
        let _ = std::fs::remove_dir_all(&escape);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    #[test]
    fn multifile_download_writes_expected_names_and_reaches_full_progress() {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for body in ["abc", "defgh"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });

        let encoder: &'static str = Box::leak(format!("http://{addr}/encoder").into_boxed_str());
        let decoder: &'static str = Box::leak(format!("http://{addr}/decoder").into_boxed_str());
        let files = [
            ModelFile {
                url: encoder,
                dest: "encoder.onnx",
                size_bytes: 3,
            },
            ModelFile {
                url: decoder,
                dest: "decoder.onnx",
                size_bytes: 5,
            },
        ];
        let dir = std::env::temp_dir().join(format!("floure-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut last = 0;
        fetch_files(&files, &dir, 8, &mut |percent, _| last = percent).unwrap();
        assert_eq!(std::fs::read(dir.join("encoder.onnx")).unwrap(), b"abc");
        assert_eq!(std::fs::read(dir.join("decoder.onnx")).unwrap(), b"defgh");
        assert_eq!(last, 100);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ultra_is_a_complete_multifile_parakeet_bundle() {
        let ultra = find_model("parakeet-ultra-int8").expect("ultra manifest");
        assert_eq!(ultra.backend, "parakeet");
        assert_eq!(ultra.files.len(), 4);
        let names: Vec<_> = ultra.files.iter().map(|file| file.dest).collect();
        assert_eq!(
            names,
            vec!["encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt"]
        );
        let total: u64 = ultra.files.iter().map(|file| file.size_bytes).sum();
        assert_eq!(total, ultra.size_bytes);
    }
}
