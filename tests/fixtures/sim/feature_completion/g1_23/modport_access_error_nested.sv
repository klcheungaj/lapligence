// llg-test-fixture: IEEE 1800-2009 25.5. Access direction is checked after
// resolving the nested member, not just the top-level modport port: a write
// through an input-only view's packed-struct member is rejected.
interface bus_if;
    struct packed { logic [7:0] lo; logic [7:0] hi; } data;
    modport slave (input data);
endinterface

module consumer(bus_if.slave s);
    always_comb s.data.lo = 8'h02;
endmodule

module tb;
    bus_if u_bus();
    consumer u_c(.s(u_bus.slave));
endmodule
