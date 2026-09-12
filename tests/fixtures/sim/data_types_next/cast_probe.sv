// IEEE 1800-2009 6.24, 7.2-7.3, and 20.5: dynamic cast and bit-stream probe.
module tb;
    logic [7:0] source;
    logic [3:0] dest;
    typedef enum logic [1:0] {IDLE = 2'b00, RUN = 2'b01} state_t;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } packed_t;
    state_t state;
    state_t copied_state;
    logic [1:0] raw_state;
    logic [3:0] destinations [0:1];
    packed_t packed_source;
    logic [7:0] packed_dest;
    real real_dest;
    integer wide_raw;
    integer status;
    initial begin
        source = 8'hab;
        dest = 4'h5;
        packed_source = '{4'hc, 4'hd};
        status = $cast(dest, source);
        $display("status=%0d dest=%h", status, dest);
        $cast(dest, 8'h3);
        $display("task dest=%h", dest);
        status = $cast(destinations[1], source);
        if (status !== 1 || destinations[1] !== 4'hb) begin
            $display("FAIL cast_probe selected destination");
            $finish;
        end
        status = $cast(packed_dest, packed_source);
        if (status !== 1 || packed_dest !== 8'hcd) begin
            $display("FAIL cast_probe packed aggregate source");
            $finish;
        end
        state = RUN;
        raw_state = 2'b10;
        status = $cast(state, raw_state);
        $display("enum_status=%0d state=%b", status, state);
        wide_raw = 4;
        state = RUN;
        status = $cast(state, wide_raw);
        if (status !== 0 || state !== RUN) begin
            $display("FAIL cast_probe wide enum value");
            $finish;
        end
        state = RUN;
        $cast(state, 2'bx0);
        if (state !== RUN) begin
            $display("FAIL cast_probe task failure changed enum");
            $finish;
        end
        state = state_t'(2'b10);
        if (state !== 2'b10) begin
            $display("FAIL cast_probe static enum coercion");
            $finish;
        end
        copied_state = RUN;
        status = $cast(copied_state, state);
        if (status !== 0 || copied_state !== RUN) begin
            $display("FAIL cast_probe invalid enum source");
            $finish;
        end
        status = $cast(real_dest, 8'd5);
        if (status !== 1 || real_dest != 5.0) begin
            $display("FAIL cast_probe real destination");
            $finish;
        end
        $display("PASS cast_probe");
        $finish;
    end
endmodule
