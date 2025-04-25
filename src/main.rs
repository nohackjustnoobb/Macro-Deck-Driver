use macro_deck_driver::MarcoDeck;

fn main() {
    let deck = MarcoDeck::new("/dev/cu.usbserial-110").unwrap();

    println!("{:?}", deck.list_directory());
}
