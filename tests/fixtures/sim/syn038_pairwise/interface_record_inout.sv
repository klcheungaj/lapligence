// IEEE 1800-2009 §§7.2 and 13.5: an interface process passes one unpacked
// record field as an inout task actual; the adjacent payload field is retained.
// The task's write to its inout formal is separate from the caller-actual path.
//
// SYN038 caller actual (TY, OP, CO, LV, SL, FM, HC, HR, CP, CT, IN, WK, PC):
//   unpacked_record, direct_projection, call_argument, field,
//   interface_member, inout, interface, local, none, task, none, none, initial.
interface record_if;
    typedef struct {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;

    record_t store;

    task automatic bump(inout logic [7:0] value);
        value = value + 8'h01;
    endtask

    initial begin
        store.key = 8'h21;
        store.payload = 8'h45;
        bump(store.key);
    end
endinterface

module tb;
    record_if bus();

    initial begin
        #1;
        if (bus.store.key !== 8'h22 || bus.store.payload !== 8'h45)
            $fatal(1, "interface inout field copy-out mismatch");
        $display("22 45");
        $finish(0);
    end
endmodule
