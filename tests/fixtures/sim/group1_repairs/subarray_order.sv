module tb;
    typedef struct {
        logic [7:0] a[0:1];
        logic [7:0] b[1:0];
        logic [7:0] c[-3:-2];
        logic [7:0] d[4:3][0:1];
        logic [7:0] e[-2:-1][7:6];
        int guard;
    } pair_t;
    pair_t s;
    initial begin
        s.guard = 99;
        s.b[1] = 8'h11;
        s.b[0] = 8'h22;
        s.a = s.b;
        if (s.a[0] !== 8'h11 || s.a[1] !== 8'h22) $fatal(1, "opposite directions");
        s.c = s.a;
        if (s.c[-3] !== 8'h11 || s.c[-2] !== 8'h22) $fatal(1, "different bases");
        s.d[4][0] = 8'h31;
        s.d[4][1] = 8'h32;
        s.d[3][0] = 8'h41;
        s.d[3][1] = 8'h42;
        s.e = s.d;
        if (s.e[-2][7] !== 8'h31 || s.e[-2][6] !== 8'h32 ||
            s.e[-1][7] !== 8'h41 || s.e[-1][6] !== 8'h42)
            $fatal(1, "multidimensional declaration order");
        s.e = s.e;
        if (s.e[-1][6] !== 8'h42 || s.guard != 99) $fatal(1, "self copy or sibling");
        s.a <= s.b;
        s.b[1] = 8'h55;
        #1;
        if (s.a[0] !== 8'h11) $fatal(1, "NBA source capture");
        $display("subarray ordering passed");
        $finish(0);
    end
endmodule
