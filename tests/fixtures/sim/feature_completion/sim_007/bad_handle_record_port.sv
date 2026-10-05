// SIM-007 boundary: a record port whose type has a class-handle member is
// legal (SV 23.2.2.3) but its value link has no change marker for the handle,
// so it is rejected at code generation like a whole class-handle port.
class box_t;
    int v;
endclass

typedef struct { string s; box_t h; } rec_t;

module child(input rec_t i, output rec_t o);
    always @* o = i;
endmodule

module tb;
    rec_t x, y;
    child u(.i(x), .o(y));
    initial begin
        x.s = "s";
        #1 $display("%s", y.s);
        $finish(0);
    end
endmodule
