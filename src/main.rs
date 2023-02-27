use std::{
    f64, format,
    fs::{self, File},
    io::Write,
    iter::Sum,
};

use itertools::{sorted, Itertools};
use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

fn pitch_frequency(key: u8) -> f64 {
    // A4 is 72 and index will be reported u7(value)
    440. * f64::powf(2., f64::from(key as i32 - 72) / 12.)
}

fn pitch_to_frametime(key: u8) -> f64 {
    1. / pitch_frequency(key)
}

fn midi_tick_to_duration(tempo: u32, tick: u32) -> f64 {
    // tempo is micro-second / quarter note
    // a quarter node is 480 ticks
    // duration will be in second
    f64::from(tempo) / 1_000_000. / 480. * f64::from(tick)
}

fn frametime_tick_to_repeat(tempo: u32, frametime: f64, tick: u32) -> u32 {
    // For monophonic or a note that is too long playing alone
    // Used for monophonic or a really long note
    (midi_tick_to_duration(tempo, tick) / frametime) as u32
}

#[derive(Clone, Copy, PartialEq)]
enum Command {
    None,
    /// `impulse 101`
    Flashlight,
    /// `slot3; slot3; slot3;...`
    SwitchScroll(u8),
    /// `slot3; slot2; slot3;...`
    SwitchGroup,
    /// `+use; wait; -use`
    Use,
    /// Ducktap in combination of `sv_gravity 11550`, `sv_gravity 11650`, or bogus with `god`  
    Ducktap,
    ///
    Nice,
    Nice2,
    Nice3,
    Stopsound,
    Attack1,
}
static mut switch: u8 = 1;
fn format_bulk(frametime: f64, repeat: u32, command: Command) -> String {
    if command == Command::Ducktap {
        format!("-----d----|------|------|{}|-|-|{}\n", frametime, repeat)
    } else if command == Command::Use {
        format!("----------|------|--u---|{}|-|-|{}\n", frametime, repeat)
    } else {
        format!(
            "----------|------|------|{}|-|-|{}|{}\n",
            frametime,
            repeat,
            match command {
                Command::None => "",
                Command::Flashlight => "impulse 100",
                Command::Nice => "speak player/sprayer",
                Command::Nice2 => "speak \"common/bodysplat(v60)\"",
                Command::Nice3 => "speak squad",
                Command::SwitchScroll(num) => match num {
                    0 => "slot0",
                    1 => "slot1",
                    2 => "slot2",
                    3 => "slot3",
                    4 => "slot4",
                    5 => "slot5",
                    _ => "",
                },
                Command::SwitchGroup => {
                    unsafe {
                        switch = switch % 2 + 1;
                        match switch {
                            1 => "slot1",
                            2 => "slot2",
                            _ => "slot0",
                        }
                    }
                }
                Command::Stopsound => "stopsound",
                Command::Attack1 => "+attack; wait; -attack",
                _ => "",
            }
        )
    }
}

fn print_events(smf: &Smf) {
    for (i, track) in smf.tracks.iter().enumerate() {
        println!("Track {}", i);
        for (j, event) in track.iter().enumerate() {
            println!("{j} : {:?}", event);
        }
    }
}

#[derive(Debug, Clone)]
struct TrackSegment {
    read_idx: usize,
    end: bool,
    key: u8,
    vel: u8,
    tick: u32,
    // stores how many seconds left until the note is done playing
    remainder: f64,
    // stores the frametime difference between current note and the next one
    quantize_remainder: f64,
    new_beat: bool,
}

impl TrackSegment {
    const fn new() -> Self {
        TrackSegment {
            read_idx: 0,
            end: false,
            key: 0,
            vel: 0,
            tick: 0,
            remainder: 0.,
            quantize_remainder: 0.,
            new_beat: false,
        }
    }
}

fn main() {
    // EDIT HERE
    let smf = Smf::parse(include_bytes!("../examples/undertale_death_by_glamour_simplified.mid")).unwrap();
    const ZEROFRAMETIME: f64 = 0.0000000000000000001;
    let sounds: Vec<Command> = vec![
        Command::Nice2,
        Command::SwitchScroll(2),
        Command::Flashlight,
        Command::Use,
    ];
    let legato = true; // play notes in succession or have like 0.002s downtime to enounciate, true = saves more line
    let mut file = File::create("foo.txt").unwrap();

    // MAYBE NO NEED TO GO FROM HERE
    const IJUSTWANTTOREAD: bool = false;
    let mut tempo = 0;
    let mut result: Vec<String> = Vec::new();
    print_events(&smf);

    let mut curr_track = 0;
    let mut track_segments: Vec<TrackSegment> = Vec::new();
    for _ in 0..smf.tracks.len() {
        track_segments.push(TrackSegment::new());
    }

    while !track_segments.iter().fold(true, |acc, e| acc && e.end) && !IJUSTWANTTOREAD {
        let curr = &mut track_segments[curr_track];

        // subarray of read_idx to continue reading from previous break
        for event in &smf.tracks[curr_track][curr.read_idx..] {
            // note is still being quantized
            if curr.remainder > 0. {
                continue;
            }

            curr.read_idx += 1;
            curr.new_beat = true;

            match event.kind {
                TrackEventKind::Meta(message) => match message {
                    MetaMessage::Tempo(temp) => tempo = u32::from(temp),
                    MetaMessage::EndOfTrack => {
                        curr.end = true;
                    }
                    _ => (),
                },
                TrackEventKind::Midi {
                    channel: _,
                    message,
                } => match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel > 0 {
                            curr.key = u8::from(key);
                        }
                        curr.vel = u8::from(vel);
                    }
                    _ => (),
                },
                _ => (),
            }

            if event.delta > (0 + legato as u32) {
                // Read until there is delta, which now starts to break into hltas for the segment
                curr.tick = u32::from(event.delta);
                curr.remainder = midi_tick_to_duration(tempo, curr.tick);
                break;
            }
        }

        curr_track = (curr_track + 1) % smf.tracks.len();

        // The first time running needs to get all of the track read first
        if !track_segments
            .iter()
            .fold(true, |acc, e| acc && (e.read_idx != 0))
        {
            continue;
        }

        // This ends the reading and writing if last read completes the read
        if track_segments.iter().fold(true, |acc, e| acc && e.end) {
            break;
        }
        // println!("write");
        let mut alt = 1;
        // Write to hltas
        // If there is one track is done quantized, loop will break.
        // Then continue reading from the previous left off index.
        while track_segments
            .iter()
            .filter(|e| !e.end && e.remainder <= 0.)
            .count()
            == 0
        {
            // See how many tracks will be worked on.
            let work_count = track_segments
                .iter()
                .filter(|e| !e.end && e.vel == 0)
                .count();

            // Attempts to add note
            // if work_count != 1 {
            track_segments
                .clone()
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.end && e.key != 0 && e.quantize_remainder <= 0.)
                .for_each(|(i, _)| {
                    if track_segments[i].vel == 0 {
                        // Because of MIDI, info in an event is applied after delta
                        // vel > 0 for that event means it is a rest.
                        // write!(file, "{}", format_bulk(ZEROFRAMETIME, 1, Command::Stopsound));
                        if !track_segments[i].new_beat && sounds[i] == Command::Ducktap {
                        } else {
                            if sounds[i] == Command::Attack1 {
                                write!(file, "{}", format_bulk(ZEROFRAMETIME, 1, Command::Attack1));
                            }
                            write!(file, "{}", format_bulk(ZEROFRAMETIME, 1, sounds[i]));
                        }

                        track_segments[i].quantize_remainder =
                            pitch_to_frametime(track_segments[i].key) - ZEROFRAMETIME;
                        track_segments[i].new_beat = false;
                    } else {
                        write!(
                            file,
                            "{}",
                            format_bulk(ZEROFRAMETIME, 1, Command::Stopsound)
                        );
                        track_segments[i].quantize_remainder = track_segments[i].remainder;
                    }
                });
            // }
            // After adding a note, it will start finding the note with lowest frame time and play that sound.
            // This loops until the one of any note is fully played, which then prompts the reader to read said track.
            // Safety: loop condition makes sure that `unwrap()` always have a value.
            let lowest_index = track_segments
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.end)
                .sorted_by(|(_, a), (_, b)| b.key.cmp(&a.key)) // for notes octaves higher
                .sorted_by(|(_, a), (_, b)| a.quantize_remainder.total_cmp(&b.quantize_remainder))
                .nth(0)
                .unwrap()
                .0;
            let mut quantize_remainder = track_segments[lowest_index].quantize_remainder;
            let frametime = pitch_to_frametime(track_segments[lowest_index].key);

            // If no notes are played, skip every notes.
            // if track_segments
            //     .iter()
            //     .fold(true, |acc, e| !e.end && e.vel > 0 && acc)
            // {
            //     // println!("{:?}", track_segments);
            //     result.push("this bulk no playing\n".to_string());
            //     result.push(format_bulk(frametime, frametime_tick_to_repeat(tempo, frametime, track_segments[lowest_index].tick), Command::None));
            //     track_segments = track_segments
            //         .iter()
            //         .map(|e| TrackSegment {
            //             remainder: e.remainder - quantize_remainder,
            //             quantize_remainder: e.quantize_remainder - quantize_remainder,
            //             ..*e
            //         })
            //         .collect_vec();

            //     break;
            // }

            // if work_count == 1 {
            //     if quantize_remainder > track_segments[lowest_index].remainder
            //         || quantize_remainder == 0.
            //     {
            //         quantize_remainder = track_segments[lowest_index].remainder;
            //     }

            //     println!("this prints {:?} {}", track_segments, quantize_remainder);
            //     // result.push("this bulk one playing\n".to_string());
            //     write!(
            //         file,
            //         "{}",
            //         format_bulk(
            //             frametime,
            //             (quantize_remainder / frametime) as u32,
            //             sounds[lowest_index],
            //         )
            //     );
            // } else
            {
                write!(
                    file,
                    "{}",
                    format_bulk(
                        track_segments[lowest_index].quantize_remainder,
                        1,
                        Command::None,
                    )
                );
            }
            // update
            track_segments = track_segments
                .iter()
                .map(|e| TrackSegment {
                    remainder: e.remainder - quantize_remainder,
                    quantize_remainder: e.quantize_remainder - quantize_remainder,
                    ..*e
                })
                .collect_vec();

            // if track_segments[lowest_index].quantize_remainder <= 0. {
            //     track_segments[lowest_index].quantize_remainder = 0.;
            // }
        }
    }

    // for i in result {
    //     // print!("{}", i)
    //     write!(file, "{}", i);
    // }
}
