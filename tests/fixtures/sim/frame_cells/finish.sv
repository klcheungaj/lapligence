// llg-test-fixture: tests/fixtures/sim/frame_cells/finish.sv
module tb;
    initial begin
        automatic integer value = 7;
        automatic real fraction = 0.5;
        $display("finish %0d %0.1f", value, fraction);
        $finish(0);
    end
endmodule
