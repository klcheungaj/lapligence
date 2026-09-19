// IEEE 1800-2009 6.19.5.3-6.19.5.4: next/prev accept at most one unsigned
// step argument; other enum methods accept none. This fixture contains one
// fault and must be rejected, never silently treated as a positive case.
module tb;
    typedef enum logic [1:0] { A, B, C } e_t;
    e_t state;
    initial begin
        state = A;
        $display("%b", state.next(1, 2));
        $finish;
    end
endmodule
