use super::*;
#[test]
fn disconnected_movie_worker_fails_even_without_a_new_frame_request() {
    let (tx, _rx) = mpsc::sync_channel(1);
    let (output, input) = mpsc::sync_channel(2);
    let mut player = Player {
        requests: Some(tx),
        responses: Some(input),
        worker: None,
        last: Some(Request {
            movie: Movie::Frontend,
            frame: 0,
        }),
    };
    assert!(player.poll().unwrap().is_empty());
    drop(output);
    assert!(
        player
            .poll()
            .err()
            .unwrap()
            .to_string()
            .contains("movie decoder stopped")
    );
}
