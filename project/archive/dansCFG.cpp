
/** 
 * Okay check it. so Im pretty sure this is the combination (courtesy of Dan's compiling of our groups work into C++)
 * that we are going to need to convert to Rust and clean up to make our "Silly Parser". 
 * 
 * 
 * Grammarparser.java is from LGA13 
 * dansCFG.cpp is from LGA18 (Im hoping)
 * 
 * Meaning I am hoping that this is the file we need to modify to make our Rust Silly Parser, and I did wayyy to much shit
 * previously in my adderall infused haze.
**/


#include <iostream>
#include <fstream>
#include <vector>
#include <set>
#include <map>
#include <iomanip>
#include <stack>
#include <algorithm>

using namespace std;

// We define production rules in the form LHS -> RHS
struct Rule {
    int id;                // Unique rule identifier (more or less line rule #)
    string lhs;            // Left-Hand Side (aka nonterminals)
    vector<string> rhs;    // Right-Hand Side (whatever the production rule evaluates to )
};

struct Node {
    string value;
    Node* parent;
    vector<Node*> children;

    Node(string v, Node* p = nullptr) {
        value = v;
        parent = p;
    }
};

struct TokenStream {
    vector<string> tokens;
    int pos = 0;

    string peek() {
        if (pos < tokens.size()) {
            return tokens[pos];
        } else {
            return "$";
        }
    }

    string pop() {
        if (pos < tokens.size()) {
            string currentToken = tokens[pos];
            pos++;
            return currentToken;
        } else {
            return "$";
        }
    }
};

// Small function to determine whether or not a char is a nonterminal
bool isNonTerminal(const string& s) {
    // lambdas, pipes, & arrows get filtered out
    if ((s == "lambda") || (s == "|") || (s == "->")) {
        return false;
    }

    // If the string contains/ is an uppercase character, it is a nonterminal
    for (int i = 0; i < s.length(); i++) {
        if ((s[i] >= 'A') && (s[i] <= 'Z')) {
            return true;
        }
    }

    // Otherwise, the string is a terminal
    return false;
}

bool derivesToLambda(const string& symbol, const vector<Rule>& grammar, set<string> visited = {}) {
    if (symbol == "lambda") {
        return true;
    }

    if (!isNonTerminal(symbol)) {
        return false;
    }

    if (visited.count(symbol)) {
        return false;
    }

    visited.insert(symbol);

    for (const Rule& rule : grammar) {
        if (rule.lhs == symbol) {
            bool allTokensDeriveLambda = true;
            
            for (const string& token : rule.rhs) {
                if (!derivesToLambda(token, grammar, visited)) {
                    allTokensDeriveLambda = false;
                    break;
                }
            }

            if (allTokensDeriveLambda) {
                return true;
            }
        }
    }

    return false;
}

set<string> firstSet(vector<string> sequence, set<string>& T, const vector<Rule>& grammar) {
    set<string> F;

    if (sequence.empty()) {
        return F;
    }

    string X = sequence[0];

    if (!isNonTerminal(X)) {
        F.insert(X);
        return F;
    }

    if (T.find(X) == T.end()) {
        T.insert(X);

        for (const Rule& p : grammar) {
            if (p.lhs == X) {
                set<string> G = firstSet(p.rhs, T, grammar);
                F.insert(G.begin(), G.end());
            }
        }
    }

    if (derivesToLambda(X, grammar) && sequence.size() > 1) {
        vector<string> beta(sequence.begin() + 1, sequence.end());
        set<string> G = firstSet(beta, T, grammar);
        F.insert(G.begin(), G.end());
    }

    return F;
}

set<string> followSet(string A, set<string>& T, const vector<Rule>& grammar) {
    if (T.find(A) != T.end()) {
        return {};
    }

    T.insert(A);
    set<string> F;

    for (const Rule& p : grammar) {
        for (int i = 0; i < p.rhs.size(); i++) {
            if (p.rhs[i] == A) {
                vector<string> pi;
                for (int j = i + 1; j < p.rhs.size(); j++) {
                    pi.push_back(p.rhs[j]);
                }

                if (!pi.empty()) {
                    set<string> tempT;
                    set<string> G = firstSet(pi, tempT, grammar);
                    F.insert(G.begin(), G.end());
                }

                bool allDeriveLambda = true;
                for (const string& s : pi) {
                    if (!derivesToLambda(s, grammar)) {
                        allDeriveLambda = false;
                        break;
                    }
                }

                if (pi.empty() || allDeriveLambda) {
                    set<string> G = followSet(p.lhs, T, grammar);
                    F.insert(G.begin(), G.end());
                }
            }
        }
    }

    return F;
}

map<pair<string, string>, int> generateParsingTable(const vector<Rule>& grammar, const set<string>& nonTerminals) {
    map<pair<string, string>, int> table;

    for (const Rule& rule : grammar) {
        set<string> visitedT;
        set<string> firstRHS = firstSet(rule.rhs, visitedT, grammar);

        for (const string& terminal : firstRHS) {
            if (terminal != "lambda") {
                table[{rule.lhs, terminal}] = rule.id;
            }
        }

        bool rhsDerivesLambda = true;
        for (const string& sym : rule.rhs) {
            if (!derivesToLambda(sym, grammar)) {
                rhsDerivesLambda = false;
                break;
            }
        }
        if (rule.rhs.empty() || (rule.rhs[0] == "lambda")) {
            rhsDerivesLambda = true;
        }

        if (rhsDerivesLambda) {
            set<string> visitedFollow;
            set<string> fSet = followSet(rule.lhs, visitedFollow, grammar);
            for (const string& terminal : fSet) {
                table[{rule.lhs, terminal}] = rule.id;
            }
        }
    }
    return table;
}

Node* LLTabularParsing(TokenStream& ts, map<pair<string, string>, int>& LLT, const vector<Rule>& P) {
    if (P.empty()) {
        return nullptr;
    }

    string S = P[0].lhs;

    Node* root = new Node("root");
    Node* current = root;
    stack<string> K;
    const string MARKER = "<*>";

    K.push(S); 

    while (!K.empty()) {
        string x = K.top();
        K.pop();

        if (x == MARKER) {
            current = current->parent;
        }
        else if (isNonTerminal(x)) {
            string lookahead = ts.peek();
            
            if (LLT.find({x, lookahead}) == LLT.end()) {
                cout << "Error: No transition for [" << x << ", " << lookahead << "]" << endl;
                return nullptr;
            }

            int ruleId = LLT[{x, lookahead}];
            const Rule& p = P[ruleId - 1];

            K.push(MARKER);
            
            for (int i = (p.rhs.size() - 1); i >= 0; i--) {
                if (p.rhs[i] != "lambda") {
                    K.push(p.rhs[i]);
                }
            }

            Node* n = new Node(x, current);
            current->children.push_back(n);
            current = n;
        }
        else {
            if (x == "lambda") {
                continue;
            }

            if (x == ts.peek()) {
                string tokenValue = ts.pop();
                current->children.push_back(new Node(tokenValue, current));
            } else {
                cout << "Error: Expected " << x << " - Found: " << ts.peek() << endl;
                return nullptr;
            }
        }
    }
    
    if (root->children.empty()) {
        return nullptr;
    } else {
        return root->children[0];
    }
}

void printTree(Node* node, int depth = 0) {
    if (!node) return;
    for (int i = 0; i < depth; i++) cout << "  ";
    cout << node->value << endl;
    for (Node* child : node->children) {
        printTree(child, depth + 1);
    }
}

// Shamelessly stolen from Thomas's work in Java
set<string> predictSet(const Rule& rule, const vector<Rule>& grammar) {
    set<string> result;
    set<string> visitedFirst;
    set<string> firstRhs = firstSet(rule.rhs, visitedFirst, grammar);

    for (const string& s : firstRhs) {
        if (s != "lambda") {
            result.insert(s);
        }
    }

    // I'm a hypocrite lol
    bool rhsDerivesLambda = (firstRhs.find("lambda") != firstRhs.end()) || rule.rhs.empty();
    
    if (!rule.rhs.empty() && (rule.rhs[0] == "lambda")) {
        rhsDerivesLambda = true;
    }

    if (rhsDerivesLambda) {
        set<string> visitedFollow;
        set<string> fLhs = followSet(rule.lhs, visitedFollow, grammar);
        result.insert(fLhs.begin(), fLhs.end());
    }

    return result;
}

// Also shamelessly stolen from Thomas
bool isPairwiseDisjoint(const set<string>& nonTerminals, const vector<Rule>& grammar) {
    bool overallDisjoint = true;

    for (const string& nt : nonTerminals) {
        vector<Rule> ntRules;
        for (const Rule& r : grammar) {
            if (r.lhs == nt) {
                ntRules.push_back(r);
            }
        }

        if (ntRules.size() < 2) {
            continue;
        }

        vector<set<string>> predicts;
        for (const Rule& r : ntRules) {
            predicts.push_back(predictSet(r, grammar));
        }

        bool ntDisjoint = true;
        for (int i = 0; i < ntRules.size(); i++) {
            for (int j = i + 1; j < ntRules.size(); j++) {
                
                set<string> intersection;
                set_intersection(predicts[i].begin(), predicts[i].end(),
                                 predicts[j].begin(), predicts[j].end(),
                                 inserter(intersection, intersection.begin()));

                if (!intersection.empty()) {
                    ntDisjoint = false;
                    overallDisjoint = false;
                    cout << "  CONFLICT between rule (" << ntRules[i].id 
                         << ") and rule (" << ntRules[j].id << "): shared tokens = { ";

                    for (const string& s : intersection) {
                        cout << s << " ";
                    }

                    cout << "}" << endl;
                }
            }
        }

        if (ntDisjoint) {
            cout << "  " << nt << ": PREDICT sets are pairwise disjoint (LL(1) ok)" << endl;
        } else {
            cout << "  " << nt << ": PREDICT sets are NOT pairwise disjoint (not LL(1))" << endl;
        }
    }

    return overallDisjoint;
}

int main(int argc, char* argv[]) {
    ifstream file(argv[1]);
    string token;
    vector<string> tokens;

    // All strings are read in from the file into a flat vector for easier look-ahead logic later on
    while (file >> token) {
        tokens.push_back(token);
    }

    // Defining all the goodies we'll use when parsing
    vector<Rule> grammar;
    set<string> nonTerminals;
    set<string> allSymbols;
    set<string> terminals;
    string startSymbol = "";
    string currentLHS = "";
    int ruleCount = 1;

    // Start of the actual rule parsing
    for (int i = 0; i < tokens.size(); i++) {
        // Check if current token is the start of a new rule definition (ex: "A ->")
        if ((i + 1 < tokens.size()) && (tokens[i+1] == "->")) {
            currentLHS = tokens[i];
            i++; // Skip arrows since they are not in our grammar
        }
        else {
            // We can assume that a production has been found for the current LHS. Therefore, we make a new Rule struct
            Rule newRule;
            newRule.id = ruleCount++;
            newRule.lhs = currentLHS;
            
            nonTerminals.insert(currentLHS);
            allSymbols.insert(currentLHS);

            // Populate the RHS until we hit a pipe or the next rule definition
            while (i < tokens.size()) {
                if (tokens[i] == "|") {
                    break; // End of current production, next will use same LHS
                }
                if ((i + 1 < tokens.size()) && (tokens[i+1] == "->")) {
                    i--; // Backtrack so the outer loop captures the next LHS
                    break;
                }
                
                newRule.rhs.push_back(tokens[i]);
                
                if (tokens[i] != "lambda") {
                    allSymbols.insert(tokens[i]);

                    if (!isNonTerminal(tokens[i])) {
                        terminals.insert(tokens[i]);
                    }

                    // Rule for finding the Goal/ Start symbol (marked by '$').
                    // There's a number of ways you could implement this more easily if you can assert that the first production
                    // rule always has your start symbol, but I wasn't sure if that assumption was permissible
                    if (tokens[i] == "$") {
                        startSymbol = currentLHS;
                    }
                }
                i++;
            }

            // We update our existing set of grammar rules
            grammar.push_back(newRule);
        }
    }

    // And finally, the code for printing out our findings. I tried to style this as closely to Keith's
    // sample outputs as possible, but I believe he parsed his rules differently so the order of terminals &
    // nonterminals is inconsistent. Otherwise I believe it matches his style
    cout << "Grammar Non-Terminals:" << endl;
    for (set<string>::iterator it = nonTerminals.begin(); it != nonTerminals.end(); it++) {
        cout << *it;
        set<string>::iterator nextIt = it;
        nextIt++;
        if (nextIt != nonTerminals.end()) {
            cout << ", ";
        }
    }

    cout << "\n\nGrammar Symbols:" << endl;
    for (set<string>::iterator it = allSymbols.begin(); it != allSymbols.end(); it++) {
        cout << *it;
        set<string>::iterator nextIt = it;
        nextIt++;
        if (nextIt != allSymbols.end()) {
            cout << ", ";
        }
    }

    cout << "\n\nGrammar Rules:" << endl;
    for (int i = 0; i < grammar.size(); i++) {
        const Rule& r = grammar[i];
        cout << "(" << r.id << ") " << r.lhs << " -> ";
        for (int j = 0; j < r.rhs.size(); j++) {
            cout << r.rhs[j] << " ";
        }
        cout << "\n";
    }

    cout << "\nGrammar Start Symbol or Goal: " << startSymbol << endl;

    cout << "\nFirst Sets for Nonterminals:" << endl;
    for (const string& nt : nonTerminals) {
        set<string> visitedT;
        vector<string> sequence = {nt};
        
        set<string> resultF = firstSet(sequence, visitedT, grammar);

        cout << "FIRST(" << nt << ") = { ";
        for (set<string>::iterator it = resultF.begin(); it != resultF.end(); it++) {
            cout << *it;
            set<string>::iterator next_it = it;
            next_it++;
            if (next_it != resultF.end()) {
                cout << ", ";
            }
        }
        cout << " }" << endl;
    }

    cout << "\nFollow Sets for Nonterminals:" << endl;
    for (const string& nt : nonTerminals) {
        set<string> visitedT;
        set<string> resultF = followSet(nt, visitedT, grammar);

        cout << "FOLLOW(" << nt << ") = { ";
        for (set<string>::iterator it = resultF.begin(); it != resultF.end(); it++) {
            cout << *it;
            set<string>::iterator nextIt = it;
            nextIt++;
            if (nextIt != resultF.end()) {
                cout << ", ";
            }

        }
        cout << " }" << endl;
    }

    // Also also, shamelessly inspired by Thomas
    cout << "\nPredict Sets for Productions:" << endl;
    for (const Rule& r : grammar) {
        set<string> predict = predictSet(r, grammar);
        cout << "PREDICT(" << r.id << ") [" << r.lhs << " -> ";

        for (const string& s : r.rhs) {
            cout << s << " ";
        }

        cout << "] = { ";

        for (set<string>::iterator it = predict.begin(); it != predict.end(); it++) {
            cout << *it;

            if (next(it) != predict.end()) {
                cout << ", ";
            }
        }

        cout << " }" << endl;
    }

    cout << "\nPairwise Disjoint Check (LL(1) condition):" << endl;
    bool isLL1 = isPairwiseDisjoint(nonTerminals, grammar);
    if (isLL1) {
        cout << "\nGrammar is " << "" << "LL(1)." << endl;
    } else {
        cout << "\nGrammar is " << "NOT " << "LL(1)." << endl;
    }

    map<pair<string, string>, int> parsingTable = generateParsingTable(grammar, nonTerminals);

    cout << "\nLL(1) Parsing Table:" << endl << " ";
    for (const string& t : terminals) cout << setw(5) << t;
    cout << endl << string(5 + terminals.size() * 5, '-') << endl;

    for (const string& nt : nonTerminals) {
        cout << nt;
        for (const string& t : terminals) {
            if (parsingTable.count({nt, t})) {
                cout << setw(5) << parsingTable[{nt, t}];
            } else {
                cout << setw(5) << "-";
            }
        }
        cout << endl;
    }

    // This really ought to be commandline arg'd as well, but I ran out of time =/
    TokenStream input;
    cout << "\nEnter (or redirect tbh) tokens to parse:" << endl;
    
    string line;
    while (getline(cin, line) && !line.empty()) {
        stringstream ss(line);
        string tempToken;

        while (ss >> tempToken) {
            input.tokens.push_back(tempToken);
        }
    }

    if (input.tokens.empty() || (input.tokens.back() != "$")) {
        input.tokens.push_back("$");
    }

    cout << "\nStarting LL(1) Parsing..." << endl;
    Node* parseTreeRoot = LLTabularParsing(input, parsingTable, grammar);

    if (parseTreeRoot != nullptr) {
        cout << "\nParse Tree:" << endl;
        printTree(parseTreeRoot);
    } else {
        cout << "\nThe input sentence is not valid for this grammar." << endl;
    }



    return 0;
}