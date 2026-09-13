// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/parameter_mismatch.sv
// IEEE 1800-2009 §25.9: virtual-interface assignments retain nominal
// parameter specialization and reject incompatible interface widths.
interface bus_if #(parameter int W = 4);
    logic [W-1:0] data;
endinterface

module tb;
    bus_if #(8) wide();
    virtual bus_if #(4) narrow;

    initial begin
        narrow = wide;
        $finish;
    end
endmodule
