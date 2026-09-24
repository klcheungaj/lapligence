// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv
interface runtime_init_if;
    bit [7:0] value;
endinterface

module tb;
    runtime_init_if bus();
    bit [7:0] local_seed;
    logic [7:0] same_scope_copy = local_seed;
    logic [7:0] copy = bus.value;

    initial begin
        #0;
        if (copy !== 8'h00 || same_scope_copy !== 8'h00 || local_seed !== 8'h00 ||
            bus.value !== 8'h00)
            $fatal(1, "module declaration initializer source mismatch");

        local_seed = 8'h34;
        bus.value = 8'h5a;
        if (copy !== 8'h00 || same_scope_copy !== 8'h00 || local_seed !== 8'h34 ||
            bus.value !== 8'h5a)
            $fatal(1, "module declaration initializer did not capture source value");

        $display("copy=%h,source=%h,control=%h", copy, bus.value, same_scope_copy);
        $finish(0);
    end
endmodule
