// SIM-007: nonblocking writes of whole tagged unions with string, real,
// record and class-handle members (SV 7.3.2, 10.4.2): the value is fixed at
// issue, and the member, the reset of inactive members and the tag are
// queued together, so they commit in issue order.
class box_c;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass
typedef struct { string s; int n; } rec_t;
typedef union tagged { void None; int I; string S; real F; rec_t Rec; box_c H; } val_t;

module tb;
    val_t v, w, p1, p2;
    val_t arr[2];
    string src;
    box_c b;
    logic clk = 0;
    int cnt = 0;

    function automatic val_t make(string s);
        return tagged S s;
    endfunction

    always @(posedge clk) begin
        cnt <= cnt + 1;
        p1 <= tagged I cnt;
        p2 <= p1;
    end

    initial begin
        src = "first";
        v = tagged I 1;
        v <= tagged S src;
        src = "changed";
        $display("1 %0d", v.I);
        #1 $display("2 %s", v.S);
        w = tagged F 2.5;
        v <= w;
        w = tagged None;
        #1 $display("3 %0.1f", v.F);
        v <= tagged Rec '{"r", 4};
        v <= tagged I 9;
        #1 $display("4 %0d", v.I);
        b = new(3);
        v <= tagged H b;
        #1 $display("5 %0d", v.H.v);
        v <= make("call");
        #1 $display("6 %s", v.S);
        arr[1] = tagged S "e";
        v <= arr[1];
        #1 $display("7 %s", v.S);
        v = tagged I 1;
        w = tagged S "x";
        v <= w;
        w <= v;
        #1 $display("8 %s %0d", v.S, w.I);
        repeat (3) begin
            #1 clk = 1;
            #1 clk = 0;
        end
        if (p2 matches tagged I .n) $display("9 %0d", n);
        if (p1 matches tagged I .n) $display("10 %0d", n);
        $finish(0);
    end
endmodule
