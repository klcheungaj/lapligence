// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_member_child_port.sv
// A mutable parent-interface member feeds a nested child-interface input port.
interface child_if(input logic [7:0] member);
    logic [7:0] readback;
    assign readback = member;
endinterface

interface parent_if;
    logic [7:0] member;
    child_if nested(member);
endinterface

module tb;
    parent_if bus();

    initial begin
        bus.member = 8'h12;
        #1;
        if (bus.member !== 8'h12 || bus.nested.readback !== 8'h12)
            $fatal(1, "nested interface port missed first parent-member update");

        bus.member = 8'ha5;
        #1;
        if (bus.member !== 8'ha5 || bus.nested.readback !== 8'ha5)
            $fatal(1, "nested interface port missed second parent-member update");

        $display("interface-child-port=%h,%h", bus.member, bus.nested.readback);
        $finish(0);
    end
endmodule
