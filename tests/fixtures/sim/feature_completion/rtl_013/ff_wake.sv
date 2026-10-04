// IEEE 1800-2009 9.2.2.4: always_ff has one event control and no blocking
// timing, so it runs only on that event: data changes alone never wake it.
// Legal bodies include an or-list with an asynchronous reset, an iff
// qualifier, blocking stores to local data, timing-free task and function
// calls, delayed nonblocking assignments, event triggers and force/release
// (overrides are not ordinary assignments, so the forced net keeps its own
// continuous driver).
module tb;
    logic clk, rst_n, en, f;
    logic [7:0] d, q1, q2, q3, q4, src;
    wire [7:0] r;
    event done;
    int n1 = 0, n2 = 0, n3 = 0, seen = 0;

    task automatic bump(inout logic [7:0] v);
        v = v + 1;
    endtask
    function automatic logic [7:0] twice(input logic [7:0] v);
        logic [7:0] t;
        t = v;
        return t + t;
    endfunction

    assign r = src;
    always_ff @(posedge clk or negedge rst_n)
        if (!rst_n) q1 <= 0;
        else begin
            q1 <= twice(d);
            n1 <= n1 + 1;
        end
    always_ff @(posedge clk iff en) begin
        q2 <= d;
        n2 <= n2 + 1;
    end
    always_ff @(posedge clk) begin
        logic [7:0] tmp;
        tmp = d;
        bump(tmp);
        q3 <= tmp;
        q4 <= #3 tmp;
        n3 <= n3 + 1;
        -> done;
        if (f) force r = 8'haa;
        else release r;
    end
    always @(done) seen = seen + 1;

    task automatic show(input string tag);
        $display("%s %0d %0d %0d %0d | %0d %0d %0d %0d | %h", tag, q1, q2, q3, q4, n1, n2,
                 n3, seen, r);
    endtask

    initial begin
        clk = 0;
        rst_n = 1;
        en = 0;
        f = 1;
        d = 3;
        src = 8'h01;
        #1 rst_n = 0;
        #1 show("t2");
        rst_n = 1;
        d = 4;
        en = 1;
        src = 8'h02;
        #1 show("t3");
        clk = 1;
        #1 show("t4");
        src = 8'h03;
        d = 5;
        #1 show("t5");
        clk = 0;
        en = 0;
        f = 0;
        d = 6;
        #1 clk = 1;
        #1 show("t7");
        #3 show("t10");
        $finish(0);
    end
endmodule
