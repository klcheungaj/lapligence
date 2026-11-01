// Decision A20-D21: vpiStrengthVal of nets. A net bit with a known value
// reports its strength in both s0 and s1 (We1: logic 1, s0 = s1 =
// vpiWeakDrive); an unknown value (x, L, H) reports logic vpiX with the
// strength of its 1 part in s0 and of its 0 part in s1, vpiHiZ for a missing
// part (StL: s0 = vpiHiZ, s1 = vpiStrongDrive; 53X: s0 = vpiWeakDrive, s1 =
// vpiPullDrive); z reports vpiHiZ in both; a variable reads strong. A
// cbValueChange record with vpiStrengthVal runs on strength-only changes
// too (St1 -> We1 -> St1), a vpiBinStrVal record only on value changes.
// Some simulators also run the vpiBinStrVal record on strength-only
// changes; llg follows 38.36.1, which singles out vpiStrengthVal records.
//
// IEEE 1800-2009 38.15 (SystemVerilog-1800-2009.txt L62608-L62611): "typedef
// struct t_vpi_strengthval { PLI_INT32 logic; /* vpi[0,1,X,Z] */ PLI_INT32
// s0, s1; /* refer to strength coding in Annex K */"; L62675-L62677: "If the
// format field in the s_vpi_value structure is set to vpiStrengthVal, the
// value.strength pointer shall point to an array of s_vpi_strengthval
// structures. ... If the object is a reg or variable, the strength will
// always be returned as strong."; 38.36.1 L64771-L64772: "If a cbValueChange
// callback is registered and the format is set to vpiStrengthVal, then the
// callback shall occur whenever the object changes strength, including
// changes that do not result in a value change." Which part s0 and s1 give
// for an unknown value is not stated; llg follows industry-standard tools.
//
// Build A20-D21_strength_values.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d21()` declared as a system task).
`timescale 1ns/1ns
module tb;
    reg en = 1'bx, d = 1, en1 = 1, en2 = 1;
    wire w1, wl, wh, wx, wz, w;
    bufif1 (weak0, weak1) g1 (w1, 1'b1, 1'b1);
    bufif1 gl (wl, 1'b0, en);
    bufif1 gh (wh, 1'b1, en);
    assign (pull0, weak1) wx = 1'bx;
    bufif1 (strong0, strong1) s1 (w, d, en1);
    bufif1 (weak0, weak1) s2 (w, d, en2);
    initial begin
        #1 $d21;
        #1 en1 = 0;
        #1 en1 = 1;
        #1 d = 0;
        #1 $display("w=%v", w);
        $finish;
    end
endmodule
