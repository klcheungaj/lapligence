// SIM-007-A03: a tagged union with string, record, class-handle and void
// members crosses input and output ports. The links copy the tag and member
// storage and re-run on a tag change or a member-only change.
class Box;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

typedef struct { string n; int k; } rec_t;
typedef union tagged { int I; string S; rec_t Rec; Box H; void None; } val_t;

module stage(input val_t i, output val_t o, output int n);
    always @* begin
        o = i;
        n = -1;
        case (i) matches
            tagged I .x: o = tagged I (x * 2);
            tagged S .s: n = s.len();
            default: ;
        endcase
    end
endmodule

module tb;
    val_t a, b;
    int n;
    Box bx;
    stage u(.i(a), .o(b), .n(n));
    initial begin
        bx = new(5);
        a = tagged I 3;
        #1 $display("%0d %0d", b.I, n);
        a = tagged S "four";
        #1 $display("%s %0d", b.S, n);
        a = tagged Rec '{"r", 2};
        #1 $display("%s %0d %0d", b.Rec.n, b.Rec.k, n);
        a.Rec.k = 9;
        #1 $display("%0d", b.Rec.k);
        a = tagged H bx;
        #1 $display("%0d", b.H.v);
        a = tagged None;
        #1 if (b matches tagged None) $display("none");
        $finish(0);
    end
endmodule
