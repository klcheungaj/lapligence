// IEEE 1800-2009 9.2.2.2.1 and IEEE 1364-2001 9.7.5: always_comb is sensitive
// to storage read inside the functions it calls (transitively), while @*
// considers only the expressions written at the call site. A constant array
// selector reads one element; a runtime selector reads the longest static
// prefix, the whole array. Counters are declaration-initialized so each
// process remains their only procedural writer, and only changes after the
// first sample are compared, so time-zero ordering does not matter.
module tb;
    logic [7:0] g, h, arg;
    logic [7:0] comb_out, star_out;
    int comb_n = 0, star_n = 0;
    logic [7:0] mem [0:7];
    logic [2:0] i;
    logic [7:0] c_out, v_out;
    int c_n = 0, v_n = 0;
    int base_comb, base_star, base_c, base_v;

    function automatic logic [7:0] inner_f(input logic [7:0] a);
        return a + g;
    endfunction
    function automatic logic [7:0] outer_f(input logic [7:0] a);
        return inner_f(a) ^ h;
    endfunction

    always_comb begin
        comb_out = outer_f(arg);
        comb_n = comb_n + 1;
    end
    always @* begin
        star_out = outer_f(arg);
        star_n = star_n + 1;
    end
    always_comb begin
        c_out = mem[2];
        c_n = c_n + 1;
    end
    always_comb begin
        v_out = mem[i];
        v_n = v_n + 1;
    end

    task automatic show(input string tag);
        $display("%s %0d %0d +%0d +%0d | %0d %0d +%0d +%0d", tag, comb_out, star_out,
                 comb_n - base_comb, star_n - base_star, c_out, v_out, c_n - base_c,
                 v_n - base_v);
    endtask

    initial begin
        g = 1;
        h = 0;
        arg = 2;
        for (int k = 0; k < 8; k++) mem[k] = 8'(k * 3);
        i = 1;
        #1 base_comb = comb_n;
        base_star = star_n;
        base_c = c_n;
        base_v = v_n;
        show("t1");
        g = 5;
        #1 show("t2");
        h = 8'h10;
        #1 show("t3");
        arg = 3;
        #1 show("t4");
        mem[3] = 100;
        #1 show("t5");
        mem[2] = 50;
        #1 show("t6");
        mem[1] = 77;
        #1 show("t7");
        i = 3;
        #1 show("t8");
        $finish(0);
    end
endmodule
