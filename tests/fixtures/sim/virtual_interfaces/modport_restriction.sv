// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/modport_restriction.sv
// IEEE 1800-2009 §25.5: a virtual modport view permits only its declared
// directions and imported subroutines.
interface bus_if;
    logic input_data;
    logic output_data;
    task set_output(input logic value);
        output_data = value;
    endtask
    modport master(input input_data, output output_data, import set_output);
endinterface

module tb;
    bus_if bus();
    virtual bus_if.master master;

    initial begin
        master = bus;
        bus.input_data = 1'b1;
        master.set_output(1'b0);
        $display("input=%0d output=%0d", bus.input_data, bus.output_data);
        $finish;
    end
endmodule
