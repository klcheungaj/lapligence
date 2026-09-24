// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_record_constref.sv
// IEEE 1800-2009 §§7.2, 13.5.2, and 25.2: an interface initial process
// passes its unpacked record member to a read-only const-ref function formal.
interface record_read_if;
    typedef struct {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;

    record_t store;
    logic [7:0] readback;

    // Focal vector: unpacked_record, direct_projection, call_argument, field,
    // interface_member, const_ref, interface, local, none, function, none,
    // procedural_blocking, initial. The whole variable actual follows its
    // prior field writes to the same outer record.
    function automatic logic [7:0] read_key(const ref record_t value);
        return value.key;
    endfunction

    initial begin
        store.key = 8'h3C;
        store.payload = 8'hA5;
        readback = read_key(store);
    end
endinterface

module tb;
    record_read_if bus();

    initial begin
        #1;
        if (bus.readback !== 8'h3C)
            $fatal(1, "interface record const-ref function returned the wrong key");
        $display("read=%h", bus.readback);
        $finish(0);
    end
endmodule
