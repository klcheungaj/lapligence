// IEEE 1800-2009 9.2.2.2-9.2.2.4, 6.5 and 10.3: 65,537-cell (descriptor-backed)
// arrays take part in always_comb/@*/always_ff sensitivity and writer rules
// without per-cell expansion. A constant element read wakes only on that
// element; a runtime selector or called function reads the whole array; an
// always_comb that writes one element and reads the array still wakes on the
// others. Rows of a two-dimensional descriptor array are separate writers
// whatever their width: a continuous row, an always_comb row, an always_ff
// cell of another row and an output port bound to a third row coexist.
module row_source(output logic [7:0] o [65537], input logic [7:0] seed);
    always_comb begin
        for (int k = 0; k < 65537; k++) o[k] = seed;
    end
endmodule

module tb;
    localparam int N = 65537;
    logic [7:0] big [0:N-1];
    logic [7:0] cpy [0:N-1];
    logic [7:0] dst [N];
    logic [7:0] src [N];
    logic [7:0] two [4][N];
    logic [16:0] i;
    logic [7:0] c_out, v_out, s_out, f_out, d, y, x, seed;
    logic clk;
    int c_n = 0, v_n = 0, y_n = 0;
    int bc, bv, by;

    function automatic logic [7:0] peek(input logic [16:0] k);
        return big[k];
    endfunction

    always_comb begin
        c_out = big[N-1];
        c_n = c_n + 1;
    end
    always_comb begin
        v_out = big[i];
        v_n = v_n + 1;
    end
    always @* s_out = big[i] + 8'd1;
    always_comb f_out = peek(i);
    always_comb cpy = big;
    always_ff @(posedge clk) big[i] <= d;

    always_comb begin
        dst[0] = x;
        y = dst[i];
        y_n = y_n + 1;
    end

    assign two[0] = src;
    always_comb two[1] = src;
    always_ff @(posedge clk) two[2][0] <= d;
    row_source rows(.o(two[3]), .seed(seed));

    task automatic show(input string tag);
        $display("%s %h %h %h %h %h %h | +%0d +%0d +%0d | %h %h %h %h %h %h", tag, c_out,
                 v_out, s_out, f_out, cpy[5], cpy[N-1], c_n - bc, v_n - bv, y_n - by, y,
                 two[0][N-1], two[1][0], two[2][0], two[3][7], two[3][N-1]);
    endtask

    initial begin
        clk = 0;
        i = 5;
        d = 8'h33;
        x = 8'h01;
        seed = 8'h5e;
        src[0] = 8'h22;
        src[N-1] = 8'h44;
        dst[7] = 8'h77;
        #1 bc = c_n;
        bv = v_n;
        by = y_n;
        show("t1");
        clk = 1;
        #1 show("t2");
        clk = 0;
        i = N - 1;
        d = 8'h66;
        #1 show("t3");
        clk = 1;
        #1 show("t4");
        i = 7;
        dst[9] = 8'h99;
        seed = 8'h5f;
        #1 show("t5");
        x = 8'h02;
        #1 show("t6");
        $finish(0);
    end
endmodule
