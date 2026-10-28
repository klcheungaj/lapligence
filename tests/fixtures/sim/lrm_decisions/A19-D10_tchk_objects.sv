// Decision A19-D10: timing check objects. Every timing check of an instance
// is one unnamed vpiTchk, iterated from the instance in source order, with
// vpiTchkType its system task. vpiTchkRefTerm and vpiTchkDataTerm are
// vpiTchkTerm handles of the reference and data events (for `$setup` the
// second and first arguments); `$period` and `$width` have no data term
// (NULL). A term's vpiEdge holds the transitions of its edge control, with z
// counted as x (posedge: vpiPosedge, `edge[01, x1]`: vpiEdge01 | vpiEdgex1,
// none: vpiNoEdge); its vpiCondition is the `&&&` condition (NULL without
// one). vpiTchkNotifier is the notifier variable (NULL without one).
// vpiExpr iterates the events as vpiTchkTerm handles, then the notifier,
// then the other arguments (limits, threshold, conditions, delayed signals)
// in source order as their own expressions, skipping empty ones.
//
// IEEE 1800-2009 37.36 details 1-2 (SystemVerilog-1800-2009.txt
// L59948-L59951): "For the timing checks in 31.2 the relationship
// vpiTchkRefTerm shall denote the reference_event or
// controlled_reference_event, while vpiTchkDataTerm shall denote the
// data_event, if any." and "When iterating over vpiExpr from a tchk, the
// handles returned for a reference_event, a controlled_reference_event, or a
// data_event shall have the type vpiTchkTerm. All other arguments shall have
// types matching the expression." The order of vpiExpr (the notifier right
// after the terms follows industry-standard behaviour), the edge bits of an
// edge list, z as x and skipping empty arguments are llg choices.
//
// Build A19-D10_tchk_objects.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d10()` declared as a system task).
`timescale 1ns/1ns
module dut (input clk, input d, input en);
  reg notifier;
  specify
    $setup(d, posedge clk, 2, notifier);
    $hold(edge[01, x1] clk, d &&& en, 1);
    $width(negedge clk, 3, 1, notifier);
    $setuphold(posedge clk, d, 4, 5, , en, en);
  endspecify
endmodule

module tb;
  reg clk = 1'b0, d = 1'b0, en = 1'b1;
  dut u (.clk(clk), .d(d), .en(en));
  initial begin
    #1 $d10;
    $finish;
  end
endmodule
