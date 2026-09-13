// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/queue.sv
// IEEE 1800-2009 §§7.10, 25.9: a queue may store typed virtual-interface
// handles and member access follows the queue-selected instance.
interface bus_if;
    logic data;
endinterface

module tb;
    bus_if first();
    bus_if second();
    virtual bus_if handles[$];

    initial begin
        handles.push_back(first);
        handles.push_back(second);
        handles[0].data = 1'b1;
        handles[1].data = 1'b0;
        $display("first=%0d second=%0d size=%0d", first.data, second.data,
            handles.size());
        $finish;
    end
endmodule
