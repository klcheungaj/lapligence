// llg-test-fixture: IEEE 1800-2009 25.5. Two modules bound through different
// modports of ONE interface instance observe the same underlying storage: the
// producer's output view drives the interface signal, the consumer's input
// view reads it, and the interface's own continuous logic sees the same value.
interface bus_if;
    logic [7:0] data;
    logic [7:0] doubled;
    always_comb doubled = data + 8'd1;
    modport producer (output data);
    modport consumer (input data, input doubled);
endinterface

module producer(bus_if.producer p, input logic [7:0] v);
    always_comb p.data = v;
endmodule

module consumer(bus_if.consumer c, output logic [7:0] d, output logic [7:0] dd);
    always_comb begin
        d = c.data;
        dd = c.doubled;
    end
endmodule

module tb;
    logic [7:0] v;
    logic [7:0] d;
    logic [7:0] dd;
    bus_if u_bus();
    producer u_p(.p(u_bus.producer), .v(v));
    consumer u_c(.c(u_bus.consumer), .d(d), .dd(dd));

    initial begin
        v = 8'h2a;
        #1 $display("d=%h dd=%h bus=%h", d, dd, u_bus.data);
        v = 8'h40;
        #1 $display("d=%h dd=%h bus=%h", d, dd, u_bus.data);
        $finish(0);
    end
endmodule
