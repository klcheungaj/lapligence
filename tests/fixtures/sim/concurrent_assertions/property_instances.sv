// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/property_instances.sv
// IEEE 1800-2009 16.8/16.9: named sequence/property instances inherit the
// sampled clock and retain their declaration argument expansion.
module tb;
    logic clk;
    logic reset;
    logic first;
    logic second;
    logic result;

    sequence pair(x, y);
        x ##1 y;
    endsequence

    property named_check(x, y);
        @(posedge clk) x |-> y;
    endproperty

    property default_check(x, y = result);
        x |-> y;
    endproperty

    property disabled_check(x);
        disable iff (reset) x;
    endproperty

    property atom_check(x);
        x;
    endproperty

    sequence_instance: assert property (@(posedge clk) pair(first, second) |-> result)
        $display("SEQUENCE_INSTANCE_PASS");
    sequence_named_args: assert property (
        @(posedge clk) pair(.y(second), .x(first)) |-> result
    ) $display("SEQUENCE_NAMED_ARGS_PASS");
    property_instance: assert property (
        @(posedge clk) named_check(.y(result), .x(first))
    ) $display("PROPERTY_INSTANCE_PASS");
    named_default: assert property (@(posedge clk) default_check(first))
        $display("DEFAULT_ARGUMENT_PASS");
    named_disable: assert property (@(posedge clk) disabled_check(first))
        $display("DISABLE_ARGUMENT_PASS");
    property_not: assert property (@(posedge clk) not (!first))
        $display("PROPERTY_NOT_PASS");
    property_and: assert property (@(posedge clk) (first and result))
        $display("PROPERTY_AND_PASS");
    named_composition: assert property (
        @(posedge clk) (atom_check(first) and atom_check(result))
    ) $display("NAMED_COMPOSITION_PASS");
    named_or: assert property (@(posedge clk) (atom_check(first) or atom_check(result)))
        $display("NAMED_OR_PASS");
    named_not: assert property (@(posedge clk) not atom_check(!first))
        $display("NAMED_NOT_PASS");
    property_iff: assert property (@(posedge clk) (first iff result))
        $display("PROPERTY_IFF_PASS");
    property_implies: assert property (@(posedge clk) (first implies result))
        $display("PROPERTY_IMPLIES_PASS");

    initial begin
        clk = 1'b0;
        reset = 1'b0;
        first = 1'b0;
        second = 1'b0;
        result = 1'b0;
        #1 begin
            first = 1'b1;
            second = 1'b0;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            second = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
