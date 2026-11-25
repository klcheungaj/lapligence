// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/neg_string_member.sv
// Legal by IEEE 1800-2009 25.9; a string variable of an interface has no
// packed or real value, and a virtual interface does not reach it yet.
interface bus_if;
    logic [7:0] data;
    string name;
endinterface

module tb;
    bus_if bus();
    virtual bus_if v;

    initial begin
        v = bus;
        v.name = "bus";
        $display("%s", bus.name);
        $finish;
    end
endmodule
