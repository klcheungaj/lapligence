// ADV-015 checker instance identity in the VPI hierarchy (IEEE 1800-2009
// 17.3: a checker instance is a scope that holds its variables).
checker inner_chk(logic a, event clk);
    int hits = 0;
    always @(clk) hits <= hits + 1;
endchecker

checker outer_chk(logic a, event clk);
    bit [3:0] count = 0;
    always @(clk) count <= count + 1;
    inner_chk u_inner(a, clk);
endchecker

module tb;
    logic clk = 1'b0;
    logic value = 1'b1;
    outer_chk u_chk(value, posedge clk);

    initial begin
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 clk = 1'b1;
        #1;
        $vpi_checker;
        $finish(0);
    end
endmodule
