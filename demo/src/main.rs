#![no_std]
#![no_main]

extern crate alloc;

#[cfg(feature = "pthread")]
use lunacy::pthread;
use lunacy::{Args, Errno, Mode, OpenFlags};

fn demo(args: Args<'_>) -> Result<(), Errno> {
    let name = args.get(1).unwrap_or(c"Cosmopolitan").to_string_lossy();
    lunacy::println!("hello from {name}")?;
    #[cfg(target_arch = "x86_64")]
    lunacy::println!("architecture: x86_64")?;
    #[cfg(target_arch = "aarch64")]
    lunacy::println!("architecture: aarch64")?;
    lunacy::println!("pthread: {}", cfg!(feature = "pthread"))?;

    // A real libc error exercises runtime errno translation.
    assert_eq!(
        lunacy::open(c"", OpenFlags::rdonly(), Mode::empty()).err(),
        Some(Errno::ENOENT)
    );
    #[cfg(feature = "pthread")]
    let message = pthread::spawn(|| {
        let value = pthread::Mutex::new(41_u32).unwrap();
        *value.lock().unwrap() += 1;
        alloc::format!("a pthread computed {}", *value.lock().unwrap())
    })?
    .join()?;
    #[cfg(not(feature = "pthread"))]
    let message = alloc::format!("computed {} without pthreads", 42);

    let (reader, writer) = lunacy::pipe()?;
    let bytes = message.as_bytes();
    assert_eq!(lunacy::write(writer.as_fd(), bytes)?, bytes.len());
    drop(writer);
    let mut buffer = [0_u8; 128];
    let n = lunacy::read(reader.as_fd(), &mut buffer)?;
    assert_eq!(&buffer[..n], bytes);
    lunacy::println!(
        "{} (through a pipe)",
        core::str::from_utf8(&buffer[..n]).unwrap()
    )?;
    let now = lunacy::clock_gettime(lunacy::Clock::Realtime)?;
    lunacy::println!("Unix time: {}", now.tv_sec)?;
    Ok(())
}

fn main(args: Args<'_>) -> i32 {
    match demo(args) {
        Ok(()) => 0,
        Err(error) => {
            let _ = lunacy::eprintln!("demo: {error:?}");
            1
        }
    }
}

lunacy::lunacy_main!(main);
