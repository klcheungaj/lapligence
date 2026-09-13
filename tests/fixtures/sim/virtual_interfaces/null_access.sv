// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/null_access.sv
// IEEE 1800-2009 §25.9: dereferencing a null virtual-interface handle is a
// runtime failure at the access site.
interface bus_if;
    logic data;
endinterface

module tb;
    virtual bus_if handle;

    initial begin
        handle.data = 1'b1;
        $finish;
    end
endmodule
