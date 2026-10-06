// SIM-007 nearest illegal form: an automatic record declared in a
// procedural block is not a nonblocking-assignment target (SV 6.21, 10.4.2).
module tb;
    typedef struct { string s; int n; } r_t;
    initial begin
        automatic r_t r;
        r <= '{"x", 1};
        $finish(0);
    end
endmodule
