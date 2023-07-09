use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::{io, fmt::Write, hash::Hash};

pub trait Dot {
    fn dot(&self, output: &mut dyn io::Write) -> io::Result<String>;
}

pub trait DotLabel {
    fn dot_label(&self) -> String;
}

impl<T: Dot + Hash> Dot for Vec<T> {
    fn dot(&self, output: &mut dyn io::Write) -> io::Result<String> {
        let mut label = String::new();

        write!(label, "vec_{}", calculate_hash(self)).unwrap();
        write!(output, "{} [ shape = record, label = \"", label)?;

        for (i, _) in self.iter().enumerate() {
            if i != 0 {
                write!(output, "|")?;
            }

            write!(output, "<{}> {}", i, i)?;
        }

        writeln!(output, "\"];")?;

        for item in self.iter() {
            let to_label = item.dot(output)?;

            writeln!(output, "{} -> {};", label, to_label)?;
        }

        Ok(label)
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
