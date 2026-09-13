// llg-test-fixture: tests/fixtures/sim/array_sensitivity/nba_array.sv
// IEEE 1800-2009 §§7.4 and 9.4.2: an NBA element write publishes its change
// after the NBA region and wakes a reader subscribed to the selected array.
module tb;
    logic [7:0] mem [0:1];
    logic index;
    logic clk;
    logic [7:0] data;
    logic [7:0] observed;

    always_comb observed = mem[index];

    always @(posedge clk) begin
        mem[index] <= data;
    end

    initial begin
        clk = 0;
        index = 0;
        data = 8'h5a;
        mem[0] = 8'h00;
        mem[1] = 8'h11;
        #1 $display("nba0=%h", observed);

        clk = 1;
        #1 $display("nba1=%h", observed);

        index = 1;
        data = 8'ha5;
        clk = 0;
        #1;
        clk = 1;
        #1 $display("nba2=%h", observed);
        $finish;
    end
endmodule
