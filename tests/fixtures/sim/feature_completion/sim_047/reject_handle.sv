// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/reject_handle.sv
// SIM-047 A03 negative: a class handle names a collectable object; naming
// it in $dumpvars is an error (S47-D9).
class Item;
    int value;
endclass

module tb;
    Item item;
    initial begin
        item = new;
        $dumpvars(1, tb.item);
        $finish;
    end
endmodule
