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

enum Command {
    None,
    /// `impulse 101`
    Flashlight,
    /// `slot3; slot3; slot3;...`
    SwitchScroll(u8),
    /// `slot3; slot2; slot3;...`
    SwitchGroup(u8),
    /// `+use; wait; -use`
    Use(bool),
    /// Ducktap in combination of `sv_gravity 11550`, `sv_gravity 11650`, or bogus with `god`  
    Ducktap,
    ///
    Nice,
    Nice2,
    Nice3,
}

fn format_bulk(frametime: f64, repeat: u32, command: Command) -> String {
    format!(
        "----------|------|------|{}|-|-|{}|{}\n",
        frametime,
        repeat,
        match command {
            Command::None => "",
            Command::Flashlight => "impulse 100",
            Command::Nice => "speak accelerating",
            Command::Nice2 => "speak cleanup",
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
            _ => "",
        }
    )
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
        }
    }
}

fn main() {
    let smf = Smf::parse(include_bytes!("../examples/1003_fugue_split.mid")).unwrap();
    let mut tempo = 0;
    let mut result: Vec<String> = Vec::new();
    // print_events(&smf);

    let mut curr_track = 0;
    let mut track_segments: Vec<TrackSegment> = Vec::new();
    const ZEROFRAMETIME: f64 = 0.0000000000000000001;
    for _ in 0..smf.tracks.len() {
        track_segments.push(TrackSegment::new());
    }

    while !track_segments.iter().fold(true, |acc, e| acc && e.end) {
        let curr = &mut track_segments[curr_track];

        // subarray of read_idx to continue reading from previous break
        for event in &smf.tracks[curr_track][curr.read_idx..] {
            // note is still being quantized
            if curr.remainder > 0. {
                continue;
            }

            curr.read_idx += 1;

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

            if event.delta > 0 {
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

        // Write to hltas
        // If there is one track is done analyzed, loop will break.
        while track_segments
            .iter()
            .filter(|e| !e.end && e.remainder <= 0. + f64::EPSILON)
            .count()
            == 0
        {
            // For notes octaves higher and beginning of each duration
            track_segments
                .clone()
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.end && e.key != 0 && e.quantize_remainder <= 0. + f64::EPSILON)
                .for_each(|(i, _)| {
                    if track_segments[i].vel == 0 {
                        // Because of MIDI, info in an event is applied after delta
                        // vel > 0 for that event means it is a rest.
                        result.push(format_bulk(ZEROFRAMETIME, 1, {
                            match i {
                                0 => Command::Nice,
                                1 => Command::SwitchScroll(2),
                                2 => Command::Nice2,
                                3 => Command::Nice3,
                                _ => Command::None,
                            }
                        }));
                        track_segments[i].quantize_remainder =
                            pitch_to_frametime(track_segments[i].key) - ZEROFRAMETIME;
                    } else {
                        track_segments[i].quantize_remainder = track_segments[i].remainder;
                    }
                });

            // Find the track with the current lowest remainder, play the note with 0ms frame, then wait for that remainder time.
            // This loops until the tick of any note is fully decreased, which now prompt the reader to read said track.
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
            let work_count = track_segments
                .iter()
                .filter(|e| !e.end && e.vel == 0)
                .count();
            let quantize_remainder = track_segments[lowest_index].quantize_remainder;
            let frametime = pitch_to_frametime(track_segments[lowest_index].key);

            // // If this is either monophonic or just a really long note, hltas line count will be reasonably reduced
            // if work_count == 1 {
            //     // println!("this prints {} {}", frametime, work_track.key);
            //     result.push(format_bulk(
            //         frametime,
            //         frametime_tick_to_repeat(
            //             tempo,
            //             frametime,
            //             track_segments[lowest_index].tick + 1,
            //         ), // add 1 tick from meta 1-tick
            //         Command::Nice,
            //     ));

            //     // track_segments = track_segments
            //     //     .iter()
            //     //     .map(|e| TrackSegment {
            //     //         remainder: e.remainder - quantize_remainder,
            //     //         quantize_remainder: e.quantize_remainder - quantize_remainder,
            //     //         ..*e
            //     //     })
            //     //     .collect_vec();

            //     track_segments[lowest_index].remainder = 0.;
            //     track_segments[lowest_index].quantize_remainder = 0.;

            //     break;
            // }

            result.push(format_bulk(
                track_segments[lowest_index].quantize_remainder,
                1,
                Command::None,
            ));

            // update
            track_segments = track_segments
                .iter()
                .map(|e| TrackSegment {
                    remainder: e.remainder - quantize_remainder,
                    quantize_remainder: e.quantize_remainder - quantize_remainder,
                    ..*e
                })
                .collect_vec();
        }
    }

    let mut file = File::create("foo.txt").unwrap();

    for i in result {
        // print!("{}", i)
        write!(file, "{}", i);
    }
}
