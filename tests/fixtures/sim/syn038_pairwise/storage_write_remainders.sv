// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv
// IEEE 1800-2009 §§6.19, 6.21, 7.2, 7.4, 10.3.1, 10.3.2, 10.4.2, 13.5,
// and 25.2: exercise remaining legal storage/write-kind and type/storage cells.
package storage_remainders_pkg;
    typedef enum logic [1:0] { OFF = 2'b00, ON = 2'b01 } state_t;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
    typedef struct { logic [7:0] key; logic [7:0] payload; } record_t;
endpackage

interface storage_remainders_if;
    import storage_remainders_pkg::*;
    logic source;
    wire net_member;
    logic variable_member;
    state_t enum_member;

    assign net_member = source;
    assign variable_member = source;

    initial begin
        source = 1'b0;
        #1 source = 1'b1;
    end
endinterface

module storage_remainders_net_formal(output wire [7:0] value);
    logic [7:0] source;
    assign value = source;

    initial begin
        source = 8'h00;
        #1 source = 8'h55;
    end
endmodule

module storage_remainders_nba_formal(output logic [7:0] value);
    initial value <= 8'h66;
endmodule

module tb;
    import storage_remainders_pkg::*;

    storage_remainders_if bus();
    wire [7:0] net_formal_value;
    logic [7:0] nba_formal_value;
    storage_remainders_net_formal net_formal(net_formal_value);
    storage_remainders_nba_formal nba_formal(nba_formal_value);

    state_t enum_formal;
    pair_t pair_formal;
    logic [7:0] task_result_seen;
    logic [7:0] observed_return;
    logic [7:0] observed_hier_return;

    task automatic set_enum(ref state_t value);
        value = ON;
    endtask

    task automatic set_pair(ref pair_t value);
        value.hi = 4'hc;
        value.lo = 4'h3;
    endtask

    task automatic update_static_arrays(input logic verify_only);
        static logic [7:0] integral_values [0:1];
        static record_t record_values [0:1];

        if (verify_only) begin
            if (integral_values[0] !== 8'h10 || integral_values[1] !== 8'h20 ||
                record_values[0].key !== 8'h31 || record_values[0].payload !== 8'ha1 ||
                record_values[1].key !== 8'h42 || record_values[1].payload !== 8'hb2)
                $fatal(1, "fixed arrays did not retain static-local storage");
        end else begin
            integral_values[0] = 8'h10;
            integral_values[1] = 8'h20;
            record_values[0].key = 8'h31;
            record_values[0].payload = 8'ha1;
            record_values[1].key = 8'h42;
            record_values[1].payload = 8'hb2;
        end
    endtask

    task automatic schedule_static_local_nba();
        static logic [7:0] staged;
        staged <= 8'h77;
        #1;
        if (staged !== 8'h77)
            $fatal(1, "static-local NBA did not commit: %h", staged);
    endtask

    function static logic [7:0] return_slot(input logic schedule_nba,
                                            input logic initialize,
                                            input logic [7:0] value);
        if (initialize)
            return_slot = value;
        else if (schedule_nba)
            return_slot <= value;
        return return_slot;
    endfunction

    function static logic [7:0] hier_result;
    endfunction

    task automatic observe_return(input logic [7:0] value);
        if (value !== 8'h5a)
            $fatal(1, "static return-slot task actual mismatch: %h", value);
        task_result_seen = value;
    endtask

    initial begin
        set_enum(enum_formal);
        pair_formal = '0;
        set_pair(pair_formal);
        if (enum_formal !== ON || pair_formal.hi !== 4'hc || pair_formal.lo !== 4'h3)
            $fatal(1, "typed formal storage mismatch");

        bus.enum_member = ON;
        if (bus.enum_member !== ON)
            $fatal(1, "enum interface-member storage mismatch");

        update_static_arrays(1'b0);
        update_static_arrays(1'b1);

        observe_return(return_slot(1'b0, 1'b1, 8'h5a));
        if (task_result_seen !== 8'h5a)
            $fatal(1, "task did not observe the function return-slot source");
        observed_return = return_slot(1'b1, 1'b0, 8'h99);
        if (observed_return !== 8'h5a)
            $fatal(1, "return-slot NBA changed storage before commit: %h", observed_return);
        #1;
        observed_return = return_slot(1'b0, 1'b0, 8'h00);
        if (observed_return !== 8'h99)
            $fatal(1, "return-slot NBA failed to commit: %h", observed_return);

        tb.hier_result.hier_result = 8'h12;
        observed_hier_return = hier_result();
        tb.hier_result.hier_result <= 8'h34;
        if (hier_result() !== observed_hier_return)
            $fatal(1, "hierarchical return-slot NBA changed storage before commit");
        #1;
        if (hier_result() !== 8'h34)
            $fatal(1, "hierarchical return-slot NBA failed to commit: %h", hier_result());

        schedule_static_local_nba();
        #1;
        if (bus.net_member !== 1'b1 || bus.variable_member !== 1'b1)
            $fatal(1, "interface continuous net/variable assignment mismatch");
        if (net_formal_value !== 8'h55 || nba_formal_value !== 8'h66)
            $fatal(1, "output formal net/NBA mismatch: %h %h",
                   net_formal_value, nba_formal_value);

        $display("storage_write_remainders=passed");
        $finish;
    end
endmodule
