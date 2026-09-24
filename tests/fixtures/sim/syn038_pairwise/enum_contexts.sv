module tb;
    typedef enum logic [7:0] {
        PHASE_IDLE = 8'b00000001,
        PHASE_ACTIVE = 8'b00000101,
        PHASE_DONE = 8'b00000111
    } phase_t;

    int initializer_calls = 0;

    function automatic phase_t startup_phase();
        initializer_calls = initializer_calls + 1;
        startup_phase = PHASE_ACTIVE;
    endfunction

    phase_t runtime_phase = startup_phase();
    phase_t streamed_phase;
    phase_t direct_control;
    logic [7:0] stream_source;
    logic equality_hit;
    logic inside_hit;
    logic inside_miss;

    initial begin
        stream_source = 8'b00000101;
        streamed_phase = phase_t'({>>{stream_source}});
        direct_control = PHASE_IDLE;
        equality_hit = streamed_phase == PHASE_ACTIVE;
        inside_hit = streamed_phase inside {PHASE_IDLE, PHASE_ACTIVE};
        inside_miss = streamed_phase inside {PHASE_DONE};

        $display("runtime=%b calls=%0d", runtime_phase, initializer_calls);
        $display(
            "stream=%b eq=%0b inside=%0b miss=%0b direct=%b",
            streamed_phase,
            equality_hit,
            inside_hit,
            inside_miss,
            direct_control
        );
        $finish(0);
    end
endmodule
