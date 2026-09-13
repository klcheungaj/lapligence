// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/dynamic_array.sv
// IEEE 1800-2009 §§7.5, 25.9: a dynamic array may store typed virtual
// interface handles and later member accesses use the selected instance.
interface bus_if;
    logic data;
endinterface

module tb;
    bus_if first();
    bus_if second();
    virtual bus_if handles[];

    initial begin
        handles = new[2];
        handles[0] = first;
        handles[1] = second;
        handles[0].data = 1'b1;
        handles[1].data = 1'b0;
        $display("first=%0d second=%0d size=%0d", first.data, second.data,
            handles.size());
        $finish;
    end
endmodule
