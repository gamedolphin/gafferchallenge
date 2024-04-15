terraform {
  required_providers {
    hcloud = {
      source = "hetznercloud/hcloud"
      version = "~> 1.45"
    }
  }
}

variable "hcloud_token" {
  sensitive = true
}

resource "hcloud_ssh_key" "default" {
  name       = "local ssh"
  public_key = file("~/.ssh/id_ed25519.pub")
}

# Configure the Hetzner Cloud Provider
provider "hcloud" {
  token = var.hcloud_token
}

resource "hcloud_network" "network" {
  name     = "network"
  ip_range = "10.0.0.0/16"
}

resource "hcloud_network_subnet" "client-subnet" {
  type         = "cloud"
  network_id   = hcloud_network.network.id
  network_zone = "eu-central"
  ip_range     = "10.0.1.0/24"
}

resource "hcloud_network_subnet" "server-subnet" {
  type         = "cloud"
  network_id   = hcloud_network.network.id
  network_zone = "eu-central"
  ip_range     = "10.0.2.0/24"
}

resource "hcloud_network_subnet" "backend-subnet" {
  type         = "cloud"
  network_id   = hcloud_network.network.id
  network_zone = "eu-central"
  ip_range     = "10.0.3.0/24"
}

resource "hcloud_server" "client" {
  count = 0

  name        = "client-${count.index}"
  server_type = "cx11"
  image       = "ubuntu-22.04"

  ssh_keys = [hcloud_ssh_key.default.id]

  network {
    network_id = hcloud_network.network.id
    ip         = "10.0.1.${count.index+1}"
  }

  depends_on = [ hcloud_network_subnet.client-subnet, hcloud_server.server ]

  connection {
    type     = "ssh"
    user     = "root"
    host     = self.ipv4_address
    private_key = file("~/.ssh/id_ed25519")
  }

  provisioner "file" {
    source      = "../build/client"
    destination = "/client"
  }

  provisioner "file" {
    source      = "./scripts/start-client.sh"
    destination = "/start-client.sh"
  }

  provisioner "remote-exec" {
    inline = [
      "chmod a+x /client",
      "chmod a+x /start-client.sh",
      "/start-client.sh"
    ]
  }
}

resource "hcloud_server" "server" {
  count = 0

  name        = "server-${count.index}"
  server_type = "cx11"
  image       = "ubuntu-22.04"

  ssh_keys = [hcloud_ssh_key.default.id]

  network {
    network_id = hcloud_network.network.id
    ip         = "10.0.2.${count.index+1}"
  }

  depends_on = [ hcloud_network_subnet.server-subnet, hcloud_server.backend ]

  connection {
    type     = "ssh"
    user     = "root"
    host     = self.ipv4_address
    private_key = file("~/.ssh/id_ed25519")
  }

  provisioner "file" {
    source      = "../build/server"
    destination = "/server"
  }

  provisioner "file" {
    source      = "./scripts/start-server.sh"
    destination = "/start-server.sh"
  }

  provisioner "remote-exec" {
    inline = [
      "chmod a+x /server",
      "chmod a+x /start-server.sh",
      "/start-server.sh"
    ]
  }
}


resource "hcloud_server" "backend" {
  count = 0

  name        = "backend-${count.index}"
  server_type = "cx11"
  image       = "ubuntu-22.04"

  ssh_keys = [hcloud_ssh_key.default.id]

  network {
    network_id = hcloud_network.network.id
    ip         = "10.0.3.${count.index+1}"
  }

  depends_on = [ hcloud_network_subnet.backend-subnet ]

  connection {
    type     = "ssh"
    user     = "root"
    host     = self.ipv4_address
    private_key = file("~/.ssh/id_ed25519")
  }

  provisioner "file" {
    source      = "../build/backend"
    destination = "/backend"
  }

  provisioner "file" {
    source      = "./scripts/start-backend.sh"
    destination = "/start-backend.sh"
  }

  provisioner "remote-exec" {
    inline = [
      "chmod a+x /backend",
      "chmod a+x /start-backend.sh",
      "/start-backend.sh"
    ]
  }
}
