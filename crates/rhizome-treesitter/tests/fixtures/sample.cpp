#include <string>
#include <vector>

namespace myapp {

class UserService {
public:
    UserService(const std::string& name) : name_(name) {}

    std::string getName() const {
        return name_;
    }

    void setName(const std::string& name) {
        name_ = name;
    }

private:
    std::string name_;
};

struct Config {
    std::string host;
    int port;
};

enum class Status {
    Active,
    Inactive,
    Pending
};

} // namespace myapp

template <typename T>
T maxValue(T a, T b) {
    return (a > b) ? a : b;
}

void globalFunction(int x) {
    // standalone function
}
