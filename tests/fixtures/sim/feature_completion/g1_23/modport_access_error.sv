// llg-test-fixture: IEEE 1800-2009 25.5. A write to an input-only modport
// member is illegal and must be rejected before C generation.
interface bus_if;
    logic [7:0] data;
    modport slave (input data);
endinterface

module consumer(bus_if.slave s);
    always_comb s.data = 8'h01;
endmodule

module tb;
    bus_if u_bus();
    consumer u_c(.s(u_bus.slave));
endmodule
