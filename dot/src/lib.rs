use std::{io, fmt::Write};

pub trait Dot {
    fn dot(&self, output: &mut dyn io::Write, label: &str) -> io::Result<()>;
}

impl<T: Dot> Dot for Vec<T> {
    fn dot(&self, output: &mut dyn io::Write, label: &str) -> io::Result<()> {
        write!(output, "{} [ shape = record, label = \"", label)?;

        for (i, _) in self.iter().enumerate() {
            if i != 0 {
                write!(output, "|")?;
            }

            write!(output, "<{}> {}", i, i)?;
        }

        writeln!(output, "\"];")?;

        for (i, item) in self.iter().enumerate() {
            let mut new_label = String::new();
            write!(new_label, "{}:{}", label, i).unwrap();

            item.dot(output, &new_label)?;
        }

        Ok(())
    }
}