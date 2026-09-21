// llg-test-fixture: tests/fixtures/sim/syn036_capacity/cellwise_reduction.sv
// IEEE 1364-2001 §§3.3.1, 3.10 and IEEE 1800-2009 §7.4.2: a bounded fixed
// unpacked array may exceed one packed value when a reduction reads each cell.
// SYN-036 selects 65,536 cells and keeps this direct reduction cell-wise.
module tb;
    typedef logic [16:0] cell_t;
    cell_t cells [0:65535];

    initial begin
        if (cells.sum() with (17'd1) !== 17'd65536)
            $fatal(1, "cell-wise reduction capacity");
        $display("PASS syn036 cellwise reduction");
        $finish(0);
    end
endmodule
