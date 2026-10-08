use std::collections::HashMap;

pub fn get_samples() -> HashMap<String, HashMap<String, &'static [u8]>> {
    let mut samples = HashMap::new();

    samples.insert("2D Test".to_string(), {
        let mut files: HashMap<String, &'static [u8]> = HashMap::new();
        files.insert(
            "index.html".to_string(),
            include_bytes!("../../samples/2D Test/index.html") as &[u8],
        );
        files.insert(
            "preview.png".to_string(),
            include_bytes!("../../samples/2D Test/preview.png") as &[u8],
        );
        files
    });

    samples.insert("3D Test".to_string(), {
        let mut files: HashMap<String, &'static [u8]> = HashMap::new();
        files.insert(
            "index.html".to_string(),
            include_bytes!("../../samples/3D Test/index.html") as &[u8],
        );
        files.insert(
            "preview.png".to_string(),
            include_bytes!("../../samples/3D Test/preview.png") as &[u8],
        );
        files
    });

    samples.insert("Video Test".to_string(), {
        let mut files: HashMap<String, &'static [u8]> = HashMap::new();
        files.insert(
            "index.html".to_string(),
            include_bytes!("../../samples/Video Test/index.html") as &[u8],
        );
        files.insert(
            "preview.png".to_string(),
            include_bytes!("../../samples/Video Test/preview.png") as &[u8],
        );
        files.insert(
            "video.mp4".to_string(),
            include_bytes!("../../samples/Video Test/video.mp4") as &[u8],
        );
        files
    });

    samples
}
