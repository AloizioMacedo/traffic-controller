use crate::State;

pub fn sum(states: &[State]) -> i64 {
    states.iter().map(|stage| stage.duration as i64).sum()
}

pub fn cum_sum(states: &[State]) -> Vec<i64> {
    let mut cum_sum = vec![0; states.len()];

    states.iter().enumerate().fold(0, |acc, (i, state)| {
        let acc = acc + state.duration;
        cum_sum[i] = acc as i64;

        acc
    });

    cum_sum
}
