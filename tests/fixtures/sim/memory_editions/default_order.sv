// llg-test-fixture: tests/fixtures/sim/memory_editions/default_order.sv
module tb;
    reg [7:0] mem [1:0];
    initial begin
        $readmemh("words.mem", mem);
        $display("m1=%h m0=%h", mem[1], mem[0]);
    end
endmodule
