// SIM-007: unpacked tagged unions with string, real, record and class-handle
// members (SV 7.3.2, 11.9, 12.6). Each member keeps its own storage; every
// access checks the tag and patterns test the tag before the payload.
class box_c;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

typedef struct { string s; int n; } rec_t;
typedef union tagged {
    void None;
    int I;
    string S;
    real F;
    rec_t Rec;
    box_c H;
    logic [7:0] B;
} value_t;

module tb;
    value_t x, y;
    box_c b;

    initial begin
        x = tagged I 5;
        $display("1 %0d", x.I);
        x = tagged S "str";
        x.S = {x.S, "!"};
        y = x;
        x = tagged F 2.5;
        $display("2 %s %0.2f", y.S, x.F);
        if (y matches tagged S .s) $display("3 %s", s);
        if (x matches tagged S .s) $display("4 bad"); else $display("4 not S");
        if (x matches tagged F .r &&& r > 2.0) $display("5 %0.1f", r * 2.0);
        x = tagged Rec '{"rec", 7};
        x.Rec.n = x.Rec.n + 1;
        $display("6 %s %0d", x.Rec.s, x.Rec.n);
        if (x matches tagged Rec .*) $display("7 rec");
        b = new(3);
        x = tagged H b;
        x.H.v = 30;
        $display("8 %0d", b.v);
        x = tagged B 8'h5a;
        if (x matches tagged B 8'h5a) $display("9 exact");
        if (x matches tagged B .k &&& k[0] == 1'b0) $display("10 %h", k);
        case (x) matches
            tagged None: $display("11 none");
            tagged B 8'b0101????: $display("11 bad");
            default: $display("11 default");
        endcase
        casez (x) matches
            tagged B 8'b0101????: $display("12 wildcard");
            default: $display("12 default");
        endcase
        x = tagged None;
        case (x) matches
            tagged S .s: $display("13 bad %s", s);
            tagged None: $display("13 none");
        endcase
        y = tagged S "case";
        case (y) matches
            tagged I .n: $display("14 bad %0d", n);
            tagged S .s &&& s == "case": $display("14 %s", s);
        endcase
        $finish(0);
    end
endmodule
