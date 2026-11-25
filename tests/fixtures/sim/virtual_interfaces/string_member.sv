// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/string_member.sv
// IEEE 1800-2009 25.9: a virtual interface of an interface type that has a
// string variable reaches its other members; the string is used through the
// instance.
interface bus_if;
    logic [7:0] data;
    string name;
endinterface

module tb;
    bus_if bus();
    virtual bus_if v;

    initial begin
        v = bus;
        v.data = 8'd5;
        bus.name = "bus";
        $display("data=%0d name=%s", v.data, bus.name);
        $finish;
    end
endmodule
