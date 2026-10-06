// SIM-007 boundary: a class property of an unpacked record type with a string
// member is legal (SV 8.1) but class fields hold only scalar leaves, so it is
// rejected with a dedicated diagnostic (class property records are SIM-011).
typedef struct { string s; int n; } rec_t;

class holder_c;
    rec_t m;
endclass

module tb;
    holder_c h;

    initial begin
        h = new;
        h.m.s = "x";
        $display("%s", h.m.s);
        $finish(0);
    end
endmodule
