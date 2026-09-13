// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/modport_method_not_imported.sv
// IEEE 1800-2009 §25.5: a virtual modport view cannot call a method that it
// does not import.
interface bus_if;
    logic data;
    task set(input logic value);
        data = value;
    endtask
    modport monitor(input data);
endinterface

module tb;
    bus_if bus();
    virtual bus_if.monitor monitor;

    initial begin
        monitor = bus;
        monitor.set(1'b1);
        $finish;
    end
endmodule
