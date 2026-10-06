// SIM-008 boundary: a string element of a container as a scan destination is
// legal (SV 21.3.4.3) but string references have no retained element cell yet.
module tb;
    string sq[$] = '{""};
    initial void'($sscanf("ab", "%s", sq[0]));
endmodule
