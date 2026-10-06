// SIM-007 boundary: chandles shall not be ports (SV 6.14), so a record port
// with a chandle member is rejected at code generation, naming the member.
typedef struct { string s; chandle p; } rec_t;

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
