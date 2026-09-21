// llg-test-fixture: tests/fixtures/sim/memory_views/negative_outer_jump.sv
module tb;
    logic [7:0] mem [-1:-2][0:1];
    initial begin
        mem[-1][0] = 8'hee;
        mem[-1][1] = 8'hee;
        mem[-2][0] = 8'hee;
        mem[-2][1] = 8'hee;
        $readmemh("negative_jump.mem", mem);
        $display("m-2-0=%h m-2-1=%h m-1-0=%h m-1-1=%h",
                 mem[-2][0], mem[-2][1], mem[-1][0], mem[-1][1]);
    end
endmodule
