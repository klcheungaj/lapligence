// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/modport_input_write.sv
// IEEE 1800-2009 §25.5: an input modport member cannot be written through
// the virtual modport view.
interface bus_if;
    logic input_data;
    modport monitor(input input_data);
endinterface

module tb;
    bus_if bus();
    virtual bus_if.monitor monitor;

    initial begin
        monitor = bus;
        monitor.input_data = 1'b1;
        $finish;
    end
endmodule
