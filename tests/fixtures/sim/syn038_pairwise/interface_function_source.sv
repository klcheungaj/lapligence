// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_function_source.sv
// Focal source vector: TY=integral_bit_logic, OP=direct_projection,
// CO=assignment_rhs, LV=none, SL=interface_member, FM=none, HC=module,
// HR=interface_member, CP=function, CT=none, IN=none, WK=none, PC=initial.
// The focal mutable source is bus.member at `return bus.member;`; the function
// result and the caller's sampled variables are separate slots.
interface data_if;
    logic [7:0] member;
endinterface

module tb;
    data_if bus();
    data_if sibling();
    logic [7:0] first_sample;
    logic [7:0] second_sample;

    function automatic logic [7:0] read_member();
        return bus.member;
    endfunction

    initial begin
        bus.member = 8'h31;
        sibling.member = 8'h92;
        first_sample = read_member();
        if (first_sample !== 8'h31 || sibling.member !== 8'h92)
            $fatal(1, "function read did not select the bound interface member");

        bus.member = 8'h6b;
        second_sample = read_member();
        if (second_sample !== 8'h6b)
            $fatal(1, "function read did not observe the changed interface member");

        $display("member_through_function=%02h,%02h sibling=%02h",
                 first_sample, second_sample, sibling.member);
        $finish;
    end
endmodule
