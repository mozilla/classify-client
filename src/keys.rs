use serde_json::{from_str, Value};
use slog::Logger;
use std::collections::HashSet;
use std::fs::read_to_string;
use std::path::PathBuf;

pub fn load(file_path: PathBuf, app_log: Logger) -> HashSet<String> {
    let mut keys: HashSet<String> = HashSet::new();

    match read_to_string(file_path) {
        Ok(contents) => match from_str::<Value>(&contents) {
            Ok(json_value) => {
                if let Some(array) = json_value.as_array() {
                    for item in array {
                        if let Value::String(string) = &item {
                            keys.insert(string.to_string());
                        }
                    }
                }
            }
            Err(err) => {
                slog::error!(app_log, "Error parsing api keys file. {}", err)
            }
        },
        Err(err) => {
            slog::error!(app_log, "Error reading api keys file. {}", err)
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use crate::keys::load;
    use slog::Drain;
    use slog::{OwnedKVList, Record};
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    struct VecDrain {
        logs: Arc<Mutex<Vec<String>>>,
    }

    impl Drain for VecDrain {
        type Ok = ();
        type Err = slog::Never;

        fn log(&self, record: &Record, _values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
            if let Ok(mut logs) = self.logs.lock() {
                logs.push(record.level().to_string() + " / " + &record.msg().to_string());
            }
            Ok(())
        }
    }

    #[test]
    fn test_load() {
        // setup
        let logs = Arc::new(Mutex::new(Vec::new()));
        let logger =
            slog::Logger::root(slog::Fuse::new(VecDrain { logs: logs.clone() }), slog::o!());

        let missing_file: PathBuf = "./missing_file.json".into();
        let corrupt_file: PathBuf = "./corrupt_file.json".into();
        let good_file: PathBuf = "./good_file.json".into();

        let _ = fs::remove_file(missing_file.clone());
        let _ = fs::write(corrupt_file.clone(), "[\"foo\"]z");
        let _ = fs::write(good_file.clone(), "[\"foo\"]");

        // tests
        let missing_set = load(missing_file.clone(), logger.clone());
        assert!(missing_set.is_empty());
        assert!(logs
            .lock()
            .unwrap()
            .pop()
            .unwrap()
            .starts_with("ERRO / Error reading api keys file"));

        let corrupt_set = load(corrupt_file.clone(), logger.clone());
        assert!(corrupt_set.is_empty());
        assert!(logs
            .lock()
            .unwrap()
            .pop()
            .unwrap()
            .starts_with("ERRO / Error parsing api keys file"));

        let good_set = load(good_file.clone(), logger.clone());
        assert!(good_set.len() == 1);
        assert!(logs.lock().unwrap().pop().is_none());

        // cleanup
        let _ = fs::remove_file(corrupt_file);
        let _ = fs::remove_file(good_file);
    }
}
